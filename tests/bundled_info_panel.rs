//! The bundled info-panel plugin **is** thurbox's info panel.
//!
//! `src/ui/info_panel.rs` is deleted (ADR-50), so there is no native builder left to
//! compare against: what holds this plugin to the pane is the ten checked-in
//! recordings under `tests/snapshots/`, each generated from
//! `ui::info_panel::info_tree` while it existed and asserted against it in the same
//! test. That ordering is the whole of ADR-42 — a recording is only *provable* as
//! the pane's while the pane's builder is present, so it had to be taken by the
//! change before this one.
//!
//! What the recordings buy, concretely: they can fail for the reason this file
//! exists. A test that merely rendered the plugin without erroring would be
//! satisfied by a pane drawing one wrong row, which is exactly what a handover
//! deleting one side of a differential comparison leaves behind. The seven
//! acceptance snapshots cover none of it either — every one is captured with no
//! active session, at 100 columns, and this seat is carved at 120.
//!
//! **The recordings must never be regenerated from this plugin.** A
//! `cargo insta accept` here would convert ten statements about the pane into ten
//! statements about whatever the plugin currently does, and the test could no longer
//! fail for the reason it exists. If a case's recording moves, the published context
//! or the plugin moved; both are findings, neither is a snapshot to accept.
//!
//! It lives in `tests/` because it needs `plugin::PluginHost`, which under `src/`
//! only `app` may reach (`tests/architecture_rules.rs`) — an integration test is not
//! part of the library's module graph at all, so the allowlist stays untouched.
//!
//! The plugin is loaded from `src/plugin/bundled/info-panel/` directly, so this
//! checks the source that ships rather than a materialized copy.

#![cfg(feature = "plugins")]

use std::path::PathBuf;

use thurbox::plugin::{discovery, ExecutionBounds, PluginHost};
use thurbox::session::pane_context::{
    AgentMetricsSnapshot, GitSnapshot, PaneContext, SessionSnapshot, StatusSnapshot,
    SystemSnapshot, UpcomingAutomationSnapshot, UsageSnapshot, UsageWindowSnapshot,
};
use thurbox::session::view_tree::ViewNode;
use thurbox::session::{
    AgentMetrics, AgentUsage, GitStats, SessionInfo, SessionStatus, UsageWindow, WorktreeInfo,
};

/// A fixed instant, so a usage window's countdown is the same on every run. It is
/// the one input the native builder used to read for itself, and it took `now` as a
/// parameter precisely so the recordings below could be exact.
const NOW: u64 = 1_700_000_000;

/// The recorder that turns a view tree into the checked-in expectation.
///
/// A subdirectory of `tests/` is not a test target, so this is shared code with no
/// new crate: three pane oracles record a tree, and the renderer's exhaustiveness
/// is the property that keeps a compact recording whole, so it exists once. See
/// the module's own note for why the gates' source-reading helpers are copied
/// instead.
mod view_tree_record;

/// The process-wide snapshot slot is global, so every case here runs one at a
/// time — otherwise one case's publication would answer another's reader.
static SERIALIZE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// One case: the state to publish, and the recording it must reproduce.
///
/// The fields are still spelled as the *pane's* inputs — a `SessionInfo`, an
/// `AgentUsage` — rather than as a ready-made `PaneContext`, because `context` below
/// mirrors `App::build_pane_context`. Publishing a hand-written snapshot instead
/// would make a failure a statement about this fixture; deriving it the way the
/// kernel derives it keeps a failure a statement about the plugin.
struct Case {
    name: &'static str,
    info: SessionInfo,
    /// The published system section, `None` for a case with no metrics yet. Carries
    /// `thurbox_dir_bytes`, which the native pane took as a separate argument.
    metrics: Option<SystemSnapshot>,
    usage: Option<AgentUsage>,
    /// Upcoming automations as the snapshot carries them: a label and *seconds*. The
    /// native pane was handed a pre-rendered countdown string and this plugin
    /// composes its own, which is the split ADR-29 established.
    automations: Vec<UpcomingAutomationSnapshot>,
    parent: Option<&'static str>,
}

impl Case {
    /// The snapshot file's name: the case name, slugified.
    ///
    /// Derived from the name rather than written twice, so a renamed case cannot
    /// keep asserting against another case's recording.
    fn snapshot_name(&self) -> String {
        self.name.replace(' ', "-")
    }

    /// The same state as the snapshot a plugin reads.
    ///
    /// Mirrors `App::build_pane_context` — which cannot be called here, since it
    /// needs a live `App` — so it resolves the same things the kernel resolves:
    /// the clock, the repo's display name, the parent's name.
    fn context(&self) -> PaneContext {
        let session = SessionSnapshot {
            id: self.info.id.to_string(),
            name: self.info.name.clone(),
            status: StatusSnapshot::of(self.info.status),
            agent: self.info.agent.clone(),
            parent_name: self.parent.map(str::to_string),
            remote_host: self.info.remote_host.clone(),
            hook_wiring: self.info.hook_wiring.clone(),
            activity: self.info.agent_activity.clone(),
            notification: self.info.notification.clone(),
            repo_name: self
                .info
                .worktrees
                .first()
                .map(|wt| &wt.repo_path)
                .or(self.info.cwd.as_ref())
                .and_then(|p| p.file_name())
                .and_then(|f| f.to_str())
                .map(str::to_string),
            branch: self.info.worktrees.first().map(|wt| wt.branch.clone()),
            additional_dir_names: self
                .info
                .additional_dirs
                .iter()
                .filter_map(|d| d.file_name().and_then(|f| f.to_str()))
                .map(str::to_string)
                .collect(),
            git: self.info.git_stats.as_ref().map(|g| GitSnapshot {
                files_changed: g.files_changed as u64,
                insertions: g.insertions as u64,
                deletions: g.deletions as u64,
                dirty: g.dirty,
                ahead: g.ahead as u64,
                behind: g.behind as u64,
            }),
            agent_metrics: self
                .info
                .agent_metrics
                .as_ref()
                .map(|m| AgentMetricsSnapshot {
                    model_display_name: m.model_display_name.clone(),
                    cli_version: m.cli_version.clone(),
                    total_cost_usd: m.total_cost_usd,
                    total_duration_ms: m.total_duration_ms,
                    total_api_duration_ms: m.total_api_duration_ms,
                    total_lines_added: m.total_lines_added,
                    total_lines_removed: m.total_lines_removed,
                    total_input_tokens: m.total_input_tokens,
                    total_output_tokens: m.total_output_tokens,
                    context_window_size: m.context_window_size,
                    used_percentage: m.used_percentage,
                    cache_read_input_tokens: m.cache_read_input_tokens,
                    cache_creation_input_tokens: m.cache_creation_input_tokens,
                }),
            usage: self
                .usage
                .as_ref()
                .filter(|u| !u.is_empty())
                .map(|u| UsageSnapshot {
                    windows: u
                        .windows
                        .iter()
                        .map(|w| UsageWindowSnapshot {
                            label: w.label.clone(),
                            used_percent: w.used_percent,
                            resets_in_secs: w.resets_at.map(|r| r.saturating_sub(NOW)),
                        })
                        .collect(),
                    plan: u.plan.clone(),
                    note: u.note.clone(),
                }),
        };

        PaneContext {
            session: Some(session),
            // The task, file, review and session-list sections stay default
            // here: this pane reads none of them, which is itself worth pinning —
            // a pane sees only what it declares.
            tasks: Default::default(),
            review: Default::default(),
            files: Default::default(),
            session_list: Default::default(),
            system: self.metrics.clone(),
            // The pane's own automations section stays default: this plugin reads
            // the *upcoming* list, and the two readers behind one capability being
            // independently readable is what `bundled_automations_panel` pins.
            automations: Default::default(),
            upcoming_automations: self.automations.clone(),
        }
    }
}

/// An upcoming automation, as the kernel publishes one.
///
/// The `id` is fixed: neither pane drew it, and the field exists for a pane that may
/// one day *change* an automation.
fn upcoming(label: &str, due_in_secs: u64) -> UpcomingAutomationSnapshot {
    UpcomingAutomationSnapshot {
        id: 0,
        label: label.to_string(),
        due_in_secs,
    }
}

/// Everything populated: every optional row, both gauges, the agent section, two
/// usage windows, the system section and two automations.
fn full_case() -> Case {
    let mut info = SessionInfo::new("fix-osc52-tmux".to_string());
    info.status = SessionStatus::Blocked;
    info.agent = "claude".to_string();
    info.remote_host = Some("devbox".to_string());
    info.hook_wiring = Some("psmux host".to_string());
    info.agent_activity = Some(
        "reviewing a very long agent-supplied activity line that has to wrap somewhere".to_string(),
    );
    info.notification = Some("needs approval to write outside the worktree".to_string());
    info.worktrees = vec![WorktreeInfo {
        repo_path: PathBuf::from("/home/dev/thurbox"),
        worktree_path: PathBuf::from("/home/dev/wt/fix-osc52"),
        branch: "fix/osc52".to_string(),
    }];
    info.additional_dirs = vec![
        PathBuf::from("/home/dev/docs"),
        PathBuf::from("/home/dev/web"),
    ];
    info.git_stats = Some(GitStats {
        files_changed: 4,
        insertions: 213,
        deletions: 57,
        untracked: 1,
        dirty: true,
        ahead: 3,
        behind: 2,
    });
    info.agent_metrics = Some(AgentMetrics {
        model_id: Some("claude-opus".to_string()),
        model_display_name: Some("Opus".to_string()),
        total_cost_usd: Some(4.278),
        total_duration_ms: Some(7_425_000),
        total_api_duration_ms: Some(1_820_400),
        total_lines_added: Some(1_204),
        total_lines_removed: Some(318),
        total_input_tokens: Some(2_450_000),
        total_output_tokens: Some(84_300),
        context_window_size: Some(200_000),
        used_percentage: Some(37),
        current_input_tokens: Some(12),
        current_output_tokens: Some(34),
        cache_creation_input_tokens: Some(96_000),
        cache_read_input_tokens: Some(1_800_000),
        cli_version: Some("2.1.3".to_string()),
    });

    Case {
        name: "everything populated",
        info,
        metrics: Some(SystemSnapshot {
            cpu_percent: 42.7,
            memory_used: 9_663_676_416,
            memory_total: 17_179_869_184,
            session_cpu_percent: 13.25,
            session_memory_bytes: 524_288_000,
            thurbox_dir_bytes: Some(3_221_225_472),
        }),
        usage: Some(AgentUsage {
            windows: vec![
                UsageWindow {
                    label: "5h".to_string(),
                    used_percent: 63.4,
                    resets_at: Some(NOW + 9_300),
                },
                UsageWindow {
                    label: "Week".to_string(),
                    used_percent: 12.0,
                    resets_at: None,
                },
            ],
            plan: Some("max".to_string()),
            note: None,
        }),
        automations: vec![
            upcoming("shepherd-tick", 150),
            upcoming("renovate-tick", 12_000),
        ],
        parent: Some("lead"),
    }
}

/// The variants. Each drops or changes one thing, so a failure names which part
/// of the pane the plugin gets wrong rather than only that it does.
fn cases() -> Vec<Case> {
    let mut out = vec![full_case()];

    // A plain local session: no parent, no host, no hooks hint, no activity, no
    // signal, no git, no agent metrics, no usage, no automations.
    let mut bare = SessionInfo::new("demo".to_string());
    bare.status = SessionStatus::Idle;
    bare.cwd = Some(PathBuf::from("/home/dev/thurbox"));
    out.push(Case {
        name: "a bare local session",
        info: bare,
        metrics: None,
        usage: None,
        automations: Vec::new(),
        parent: None,
    });

    // Metrics present but the session is using nothing yet: the session
    // CPU/RAM rows drop out while the System section stays.
    let mut idle = full_case();
    idle.name = "no session resource usage";
    idle.metrics = Some(SystemSnapshot {
        cpu_percent: 0.0,
        memory_used: 0,
        memory_total: 0,
        session_cpu_percent: 0.0,
        session_memory_bytes: 0,
        thurbox_dir_bytes: None,
    });
    out.push(idle);

    // Usage that reports a note instead of windows — "not logged in" and the
    // like, which is a different branch of the usage section.
    let mut noted = full_case();
    noted.name = "usage with a note and no windows";
    noted.usage = Some(AgentUsage {
        windows: Vec::new(),
        plan: None,
        note: Some("not logged in".to_string()),
    });
    out.push(noted);

    // Dirty with nothing in the diff — untracked files only, which is the one
    // case that appends the " dirty" run.
    let mut untracked = full_case();
    untracked.name = "untracked changes only";
    untracked.info.git_stats = Some(GitStats {
        files_changed: 0,
        insertions: 0,
        deletions: 0,
        untracked: 2,
        dirty: true,
        ahead: 0,
        behind: 0,
    });
    out.push(untracked);

    // Exactly one changed file: the singular "1 file" branch.
    let mut one_file = full_case();
    one_file.name = "one changed file";
    one_file.info.git_stats = Some(GitStats {
        files_changed: 1,
        insertions: 9,
        deletions: 0,
        untracked: 0,
        dirty: true,
        ahead: 0,
        behind: 0,
    });
    out.push(one_file);

    // An agent reporting almost nothing: every optional metric absent but the
    // section still present, plus a sub-dollar cost and a sub-second duration —
    // the branches of `formatCost` and `formatDuration` the full case misses.
    let mut sparse = full_case();
    sparse.name = "a sparse agent report";
    sparse.info.agent_metrics = Some(AgentMetrics {
        total_cost_usd: Some(0.0423),
        total_duration_ms: Some(820),
        total_output_tokens: Some(640),
        used_percentage: Some(7),
        cli_version: Some("2.1.3".to_string()),
        ..AgentMetrics::default()
    });
    out.push(sparse);

    // A local session with usage: the heading names the plan but no host, and a
    // status whose token is not the blocked one — so the Signal row is muted.
    let mut local_usage = full_case();
    local_usage.name = "a local working session";
    local_usage.info.remote_host = None;
    local_usage.info.hook_wiring = None;
    local_usage.info.status = SessionStatus::Working;
    out.push(local_usage);

    // A single automation, due now: the `"due"` branch of the countdown.
    let mut due = full_case();
    due.name = "an automation due now";
    due.automations = vec![upcoming("flow-tick", 0)];
    out.push(due);

    // A repo with no worktree branch, so the primary repo row is the repo alone.
    let mut no_branch = full_case();
    no_branch.name = "no worktree branch";
    no_branch.info.worktrees = Vec::new();
    no_branch.info.cwd = Some(PathBuf::from("/home/dev/thurbox"));
    out.push(no_branch);

    out
}

/// Start the bundled plugin from the source that ships.
fn host() -> PluginHost {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/plugin/bundled/info-panel");
    assert!(
        dir.join("plugin.toml").is_file(),
        "the bundled plugin must live at {}",
        dir.display()
    );
    let outcome = discovery::discover_in(&[dir], None);
    assert!(
        outcome.problems.is_empty(),
        "the bundled plugin must load cleanly: {:?}",
        outcome.problems
    );
    let mut host = PluginHost::from_discovery(outcome, ExecutionBounds::default());
    assert_eq!(host.start_all(), 1, "the plugin must reach Running");
    host
}

fn render(host: &PluginHost) -> ViewNode {
    host.render_pane("info-panel", "info")
        .expect("the pane renders")
}

/// The headline claim: for every case, the pane the plugin draws is the pane the
/// recording holds — and the recording is the native pane's tree.
///
/// Equal trees paint identically (the same renderer paints both), so this is
/// byte-identity of the pane without comparing a frame, and a failure localises to a
/// line rather than to a cell.
///
/// The comparison runs through the *record* rather than through `assert_eq!` on
/// trees for the reason the recorder documents: a lost bold is one visible word on
/// one line, where two structural dumps have to be diffed by eye.
#[test]
fn the_plugin_builds_the_native_panes_view_tree() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();

    for case in cases() {
        thurbox::session::pane_context::publish(case.context());
        let plugin = render(&host);
        // Do not accept a moved snapshot. The expectation was generated from
        // `ui::info_panel::info_tree` before it was deleted (ADR-42); regenerating it
        // from this plugin would make the test unable to fail for the reason it
        // exists.
        insta::assert_snapshot!(case.snapshot_name(), view_tree_record::tree(&plugin));
    }
}

/// The recorded tree has to be non-trivial, or the assertion above could pass on an
/// empty list.
///
/// Counted from the **plugin's** tree now: the native builder that used to answer
/// this is gone, and the recording it left is what the test above compares against.
/// The numbers are unchanged, which is the point — they were true of the pane.
#[test]
fn the_compared_tree_is_a_whole_pane() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    thurbox::session::pane_context::publish(full_case().context());
    let tree = render(&host);
    let rows = match &tree {
        ViewNode::List { children: rows, .. } => rows.len(),
        other => panic!("expected a list, got {}", other.kind_name()),
    };
    assert!(
        rows > 25,
        "the fixture should exercise the whole pane: {rows}"
    );
    let gauges = count_gauges(&tree);
    assert_eq!(
        gauges, 6,
        "session CPU, the context window, two usage windows, system CPU and RAM"
    );
}

fn count_gauges(node: &ViewNode) -> usize {
    let own = usize::from(matches!(node, ViewNode::Gauge { .. }));
    own + node.children().iter().map(count_gauges).sum::<usize>()
}

/// **Enumerated divergence.** With no session open the native pane draws nothing
/// at all — `App::render_info_panel` returns before it paints a border. The
/// plugin's pane is already on screen, so it keeps drawing what it still knows:
/// the System section and any upcoming automations. A blank bordered box would be
/// worse, and a plugin cannot un-draw its own pane.
#[test]
fn with_no_session_the_plugin_still_shows_what_it_knows() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    thurbox::session::pane_context::publish(PaneContext {
        session: None,
        tasks: Default::default(),
        files: Default::default(),
        review: Default::default(),
        session_list: Default::default(),
        automations: Default::default(),
        system: Some(SystemSnapshot {
            cpu_percent: 5.0,
            memory_used: 1_073_741_824,
            memory_total: 8_589_934_592,
            session_cpu_percent: 0.0,
            session_memory_bytes: 0,
            thurbox_dir_bytes: None,
        }),
        upcoming_automations: Vec::new(),
    });
    let tree = render(&host);
    let rows = match &tree {
        ViewNode::List { children: rows, .. } => rows,
        other => panic!("expected a list, got {}", other.kind_name()),
    };
    // Divider, "System" heading, CPU gauge, RAM gauge — and nothing about a
    // session.
    assert_eq!(rows.len(), 4, "{rows:#?}");
    assert_eq!(count_gauges(&tree), 2);
}

/// Before the host has published anything, a reader answers "nothing" — so the
/// plugin's first render must produce a tree rather than an error.
#[test]
fn the_first_render_before_any_publication_succeeds() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let host = host();
    thurbox::session::pane_context::publish(PaneContext::default());
    let tree = render(&host);
    match tree {
        ViewNode::List { children: rows, .. } => assert!(rows.is_empty(), "{rows:#?}"),
        other => panic!("expected a list, got {}", other.kind_name()),
    }
}

/// The plugin must hold no surface a user's plugin could not declare: its reach
/// is exactly its manifest's capability list.
#[test]
fn the_plugin_declares_every_power_it_uses() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/plugin/bundled/info-panel");
    let outcome = discovery::discover_in(&[dir], None);
    let plugin = outcome.get("info-panel").expect("discovered");
    let declared = &plugin.manifest.capabilities;
    use thurbox::session::plugin_manifest::Capability;
    for required in [
        Capability::Render,
        Capability::Sessions,
        Capability::Metrics,
        Capability::Automations,
    ] {
        assert!(declared.contains(&required), "{required} must be declared");
    }
    // And nothing beyond them: a bundled pane that quietly asked for more would
    // stop being evidence about what a third party can build.
    assert_eq!(declared.len(), 4, "{declared:?}");
}
