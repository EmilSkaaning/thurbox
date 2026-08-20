//! What every integration test needs, in one place.
//!
//! `Published` names the whole read side of the kernel, and its own doc says
//! adding something plugins can read should not be an edit to every test. It
//! was: the struct literal was hand-written in sixteen files, `SessionRow` in
//! twenty-seven, and `fn host()` in thirteen — so a new field was a
//! sixteen-file mechanical edit and a diverging fixture row was invisible.
//! Everything here is the *shared* half; a test that needs a different fixture
//! still writes its own, usually as `..session_row(..)`.
//!
//! Rust 2021 includes this per test target via `mod common;`, so an item no
//! single target uses warns there. Hence the blanket allow — the alternative is
//! a `cfg` per helper.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use thurbox::kernel::command::InFlight;
use thurbox::kernel::host::{KeyPress, LuaHost, Published, RenderContext};
use thurbox::kernel::registry::Registry;
use thurbox::kernel::snapshot::{SessionRow, Snapshot};
use thurbox::kernel::theme::Themes;

/// The interface the binary ships, as the repository holds it.
pub fn ui_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui")
}

/// A host over the bundled interface, asserting it loaded.
///
/// The assert is the point: a plugin that failed to load renders as an absent
/// pane, so without it a test reads as "the feature is gone" rather than "the
/// file has a syntax error".
pub fn host() -> LuaHost {
    let host = LuaHost::new(ui_dir());
    assert!(
        host.error.is_none(),
        "the bundled plugins must load: {:?}",
        host.error
    );
    host
}

/// An interface built from the bundled set with `plugins` written over it, each
/// `(file name, source)`.
///
/// Returns the tempdir as well as the path: dropping it deletes the directory,
/// so a caller that lets it go loads from nothing.
pub fn interface(plugins: &[(&str, &str)]) -> (tempfile::TempDir, PathBuf) {
    let home = tempfile::tempdir().expect("tempdir");
    let ui = home.path().join("ui");
    thurbox::kernel::bundled::materialize(&ui);
    for (name, source) in plugins {
        std::fs::write(ui.join("plugins").join(name), source).expect("write");
    }
    (home, ui)
}

/// Whatever palette the environment resolves, which keeps tests independent of
/// the user's active choice.
pub fn themes() -> Themes {
    Themes::load(None)
}

/// A registry holding whatever the given host's plugins declared.
pub fn registry(host: &LuaHost) -> Registry {
    let mut registry = Registry::default();
    let (bindings, settings) = host.declarations();
    registry.declare(bindings, settings);
    registry
}

/// Publish a snapshot with the defaults every test wants.
pub fn publish(host: &LuaHost, snapshot: &Snapshot) {
    publish_with(host, snapshot, &Default::default(), &[]);
}

/// Publish with attach errors and in-flight commands spelled out.
pub fn publish_with(
    host: &LuaHost,
    snapshot: &Snapshot,
    attach_errors: &std::collections::HashMap<String, String>,
    inflight: &[InFlight],
) {
    let themes = themes();
    let registry = registry(host);
    let diffs = thurbox::kernel::diff::DiffStore::new();
    let repos = thurbox::kernel::repos::RepoStore::with_hosts(Default::default());
    host.publish(&Published {
        snapshot,
        attach_errors,
        inflight,
        themes: &themes,
        registry: &registry,
        diffs: &diffs,
        links: &Default::default(),
        content: &Default::default(),
        meta: &Default::default(),
        metrics: &Default::default(),
        status_rows: 0,
        can_open: true,
        inventory: &[],
        ui_dir: "ui",
        settings: &Default::default(),
        repos: &repos,
        wants: &Default::default(),
        focus: None,
        hovered: None,
    })
    .expect("publish");
}

/// Index of a plugin by name, naming it when it is not there.
pub fn index_of(host: &LuaHost, name: &str) -> usize {
    host.index_of(name)
        .unwrap_or_else(|| panic!("no plugin named {name}"))
}

/// A render context, the one thing a plugin is handed that it did not declare.
pub fn ctx(width: u16, height: u16, focused: bool) -> RenderContext {
    RenderContext {
        width,
        height,
        focused,
        elapsed: 1.0,
        frame: 1,
    }
}

/// The keypress a terminal would deliver for a canonical chord.
pub fn press(chord: &str) -> KeyPress {
    let mut key = KeyPress::default();
    for part in chord.split('+') {
        match part {
            "ctrl" => key.ctrl = true,
            "alt" => key.alt = true,
            "shift" => key.shift = true,
            "cmd" => key.cmd = true,
            name => {
                key.name = name.to_string();
                let mut chars = name.chars();
                key.ch = match (chars.next(), chars.next()) {
                    (Some(c), None) => Some(c),
                    _ => None,
                };
            }
        }
    }
    key
}

/// A session row with the fields nobody varies already filled in.
///
/// `SessionRow` derives no `Default` — it is in `src/`, and every field there is
/// load-bearing enough that a blanket default would be a lie — so this is the
/// fixture base. Override what the test is about:
/// `SessionRow { status: "working".into(), ..session_row("a", "one") }`.
pub fn session_row(id: &str, name: &str) -> SessionRow {
    SessionRow {
        id: id.to_string(),
        name: name.to_string(),
        agent: "claude".to_string(),
        status: "idle".to_string(),
        cwd: None,
        repo: Some("thurbox".to_string()),
        repos: vec!["thurbox".to_string()],
        member_dirs: Vec::new(),
        branch: Some("main".to_string()),
        base_branch: None,
        backend: "local-tmux".to_string(),
        backend_id: Some("%1".to_string()),
        remote_host: None,
        agent_session_id: None,
        parent_id: None,
        display_order: None,
        worktree_count: 0,
        git: None,
        shell_backend_id: None,
        hook_state: None,
    }
}

/// A `git` invocation in `dir` with the whole of `git::GIT_LOCATION_ENV`
/// scrubbed.
///
/// The same scrub `git::scrub_git_location_env` makes for thurbox's own calls,
/// and for the reason named there: git exports these to hook processes, so the
/// suite running under this project's pre-commit `cargo nextest` inherits a
/// `GIT_DIR` pointing at the real repository. `git init` then fails — and had it
/// succeeded, a fixture `commit` would have landed in *that* repository rather
/// than the tempdir.
///
/// Returned rather than run, because a caller that wants the output configures
/// its own stdio; [`git`] is the common case.
pub fn git_command(dir: &Path, args: &[&str]) -> std::process::Command {
    let mut command = std::process::Command::new("git");
    command
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null");
    for name in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_PREFIX",
        "GIT_NAMESPACE",
        // Config given on the *command line* — `git -c commit.gpgsign=true
        // commit` — travels to every child through these, hooks included, and
        // outranks the repo config a test could set. `GIT_CONFIG_GLOBAL` above
        // does not cover it. Left in place, a contributor signing this
        // project's own commits fails these tests and nothing else: there is no
        // key here to sign a fixture commit with.
        "GIT_CONFIG_PARAMETERS",
        "GIT_CONFIG_COUNT",
    ] {
        command.env_remove(name);
    }
    command
}

/// Run `git` in `dir`, asserting it succeeded and saying nothing.
pub fn git(dir: &Path, args: &[&str]) {
    let status = git_command(dir, args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?}");
}
