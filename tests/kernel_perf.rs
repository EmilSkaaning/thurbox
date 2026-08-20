//! Performance behaviour, asserted on counters rather than on the clock.
//!
//! v1 learned this the hard way: a test that says "idle should be fast" is
//! flaky on shared hardware, while one that says "an idle loop painted no
//! frames" is exact. These re-derive what ADR-P6 and ADR-P12 gave v1, against
//! the v2 render path.

use thurbox::kernel::command::{Args, Command, CommandBus};
use thurbox::kernel::host::{LuaHost, RenderContext};
use thurbox::kernel::perf::Counters;
use thurbox::kernel::snapshot::{SnapshotStore, REFRESH_INTERVAL};
use thurbox::storage::Database;

mod common;

/// This file renders nothing focused; the width and height are what vary.
fn ctx(width: u16, height: u16) -> RenderContext {
    common::ctx(width, height, false)
}

fn plugin_dir(source: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let plugins = dir.path().join("plugins");
    std::fs::create_dir_all(&plugins).expect("mkdir");
    std::fs::write(plugins.join("10_pane.lua"), source).expect("write");
    dir
}

#[test]
fn an_unchanged_tree_is_the_signal_to_skip_a_frame() {
    // The heart of demand-driven repaint: a plugin returning the same tree
    // gives the loop nothing to paint. Asserted on the trees themselves rather
    // than by driving a terminal, so it is exact.
    let dir = plugin_dir(
        r#"return { name = "static", slot = "a",
                    render = function() return { text = "unchanging" } end }"#,
    );
    let host = LuaHost::new(dir.path());

    let first = host.render(0, ctx(20, 3)).expect("render").node;
    for _ in 0..50 {
        let again = host.render(0, ctx(20, 3)).expect("render").node;
        assert_eq!(first, again, "a static plugin must return an equal tree");
    }
}

#[test]
fn a_changing_tree_is_never_equal() {
    // The other half: if this ever compared equal, the loop would stop
    // painting a pane that is actually moving.
    let dir = plugin_dir(
        r#"return { name = "clock", slot = "a",
                    render = function(ctx) return { text = "frame " .. ctx.frame } end }"#,
    );
    let host = LuaHost::new(dir.path());

    let a = host
        .render(
            0,
            RenderContext {
                frame: 1,
                ..ctx(20, 3)
            },
        )
        .expect("render")
        .node;
    let b = host
        .render(
            0,
            RenderContext {
                frame: 2,
                ..ctx(20, 3)
            },
        )
        .expect("render")
        .node;
    assert_ne!(a, b);
}

#[test]
fn counters_distinguish_painted_frames_from_skipped_ones() {
    // What the perf HUD and these tests both read.
    let counters = Counters::default();
    let before = counters.read();

    Counters::bump(&counters.iterations);
    Counters::bump(&counters.frames);
    for _ in 0..9 {
        Counters::bump(&counters.iterations);
        Counters::bump(&counters.skipped);
    }

    let window = counters.read().since(&before);
    assert_eq!(window.iterations, 10);
    assert_eq!(window.frames, 1);
    assert_eq!(window.skipped, 9);
    // The property v1's ADR-P-series existed for: idle iterations vastly
    // outnumber painted frames.
    assert!(window.skipped > window.frames * 5);
}

#[test]
fn an_idle_store_stops_querying_the_database() {
    // ADR-P6 re-derived: v1 cached hook state behind a `data_version` check to
    // keep an idle tick off the sessions table. The interval alone would re-read
    // five tables every 400 ms forever on a database nobody wrote to, so the
    // property is that a due refresh with no commit behind it does not rebuild.
    //
    // This used to time ten thousand `current()` calls against a store that owned
    // no database at all, and assert only that the clock had not moved much: it
    // measured a field read, and would have passed with the gate deleted.
    let db = Database::open_in_memory().expect("db");
    let mut store = SnapshotStore::with_database(db);
    let rows = store.current().sessions.clone();

    // Past the interval, so `data_version` is the only thing left to stop a
    // rebuild — and nothing committed.
    std::thread::sleep(REFRESH_INTERVAL + std::time::Duration::from_millis(50));
    assert!(
        !store.refresh_if_due(),
        "an idle store rebuilt anyway — the data_version gate is gone"
    );
    assert_eq!(
        store.current().sessions,
        rows,
        "and the rows a plugin reads are the ones already in memory"
    );
}

#[test]
fn dispatching_a_command_never_blocks_the_caller() {
    // ADR-P12 re-derived: v1 moved the whole new-session flow off the UI thread
    // deliberately. Here it falls out of the bus — there is no blocking form to
    // accidentally use.
    let bus = CommandBus::new();
    let started = std::time::Instant::now();

    for _ in 0..20 {
        bus.dispatch(
            Command::parse(
                "create",
                Args {
                    repo: Some("/definitely/not/a/repo".into()),
                    ..Args::default()
                },
            )
            .expect("parse"),
        );
    }

    assert!(
        started.elapsed() < std::time::Duration::from_millis(500),
        "20 dispatches took {:?}",
        started.elapsed()
    );
    assert_eq!(bus.inflight().len(), 20);
}

#[test]
fn rendering_many_panes_stays_within_a_frame_budget() {
    // A regression guard, not a benchmark: generous enough for shared CI, tight
    // enough to catch an accidental O(n²) in the render path.
    let dir = plugin_dir(
        r#"return { name = "rows", slot = "a", render = function(ctx)
             local out = {}
             for i = 1, 200 do out[i] = { type = "text", len = 1, text = "row " .. i } end
             return { type = "box", children = out }
           end }"#,
    );
    let host = LuaHost::new(dir.path());

    let started = std::time::Instant::now();
    for _ in 0..200 {
        host.render(0, ctx(80, 40)).expect("render");
    }
    let each = started.elapsed() / 200;
    assert!(
        each < std::time::Duration::from_millis(20),
        "a 200-row pane took {each:?} per render"
    );
}
