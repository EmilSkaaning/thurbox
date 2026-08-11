//! Turning a published kernel snapshot into Lua tables.
//!
//! The inbound half of the boundary: [`crate::session::pane_context`] publishes
//! plain data and this module hands a plugin a table view of it. Nothing here
//! reaches the running application — it converts a value the publisher already
//! froze, which is what lets a reader run on the plugin worker while the UI
//! thread carries on drawing.
//!
//! Absent values are **absent keys**, not `false` or `0`. A plugin then writes
//! `if session.parentName then` and gets the same shape the manifest's
//! capability model uses for a binding it was not granted: a thing you did not
//! get is simply not there.
//!
//! Numbers cross as numbers. The snapshot deliberately publishes byte counts,
//! token counts and durations raw so the plugin composes every string it draws
//! (see the module docs on `pane_context`), and this layer must not undo that by
//! stringifying on the way through.

use mlua::{Lua, Table};

use crate::session::pane_context::{
    AutomationSnapshot, FileNodeSnapshot, PaneContext, ReviewLineSnapshot, SessionRowSnapshot,
    SessionSnapshot, SystemSnapshot, TaskSnapshot,
};

/// Set `key` to `value` only when there is one.
///
/// A helper rather than an inline `if let` at thirty call sites, and it is what
/// makes "absent means absent" uniform: a field that later becomes optional gets
/// the same treatment without a second convention appearing.
fn set_opt<V: mlua::IntoLua>(table: &Table, key: &str, value: Option<V>) -> mlua::Result<()> {
    if let Some(value) = value {
        table.set(key, value)?;
    }
    Ok(())
}

/// The active session, or `Nil` when there is none.
pub fn session_table(lua: &Lua, context: &PaneContext) -> mlua::Result<mlua::Value> {
    let Some(session) = context.session.as_ref() else {
        return Ok(mlua::Value::Nil);
    };
    Ok(mlua::Value::Table(build_session(lua, session)?))
}

fn build_session(lua: &Lua, s: &SessionSnapshot) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("id", s.id.clone())?;
    t.set("name", s.name.clone())?;
    t.set("agent", s.agent.clone())?;

    let status = lua.create_table()?;
    status.set("name", s.status.name)?;
    status.set("label", s.status.label.clone())?;
    status.set("icon", s.status.icon)?;
    // The token the *kernel* resolved this status to. A plugin drawing a status
    // dot uses it rather than deriving one, so a pane cannot disagree with the
    // session list about which colour a state gets.
    status.set("token", s.status.token)?;
    t.set("status", status)?;

    set_opt(&t, "parentName", s.parent_name.clone())?;
    set_opt(&t, "remoteHost", s.remote_host.clone())?;
    set_opt(&t, "hookWiring", s.hook_wiring.clone())?;
    set_opt(&t, "activity", s.activity.clone())?;
    set_opt(&t, "notification", s.notification.clone())?;
    set_opt(&t, "repoName", s.repo_name.clone())?;
    set_opt(&t, "branch", s.branch.clone())?;

    let dirs = lua.create_table()?;
    for (i, name) in s.additional_dir_names.iter().enumerate() {
        dirs.set(i + 1, name.clone())?;
    }
    t.set("additionalDirNames", dirs)?;

    if let Some(g) = s.git.as_ref() {
        let git = lua.create_table()?;
        git.set("filesChanged", g.files_changed)?;
        git.set("insertions", g.insertions)?;
        git.set("deletions", g.deletions)?;
        git.set("dirty", g.dirty)?;
        git.set("ahead", g.ahead)?;
        git.set("behind", g.behind)?;
        t.set("git", git)?;
    }

    if let Some(m) = s.agent_metrics.as_ref() {
        let metrics = lua.create_table()?;
        set_opt(&metrics, "modelDisplayName", m.model_display_name.clone())?;
        set_opt(&metrics, "cliVersion", m.cli_version.clone())?;
        set_opt(&metrics, "totalCostUsd", m.total_cost_usd)?;
        set_opt(&metrics, "totalDurationMs", m.total_duration_ms)?;
        set_opt(&metrics, "totalApiDurationMs", m.total_api_duration_ms)?;
        set_opt(&metrics, "totalLinesAdded", m.total_lines_added)?;
        set_opt(&metrics, "totalLinesRemoved", m.total_lines_removed)?;
        set_opt(&metrics, "totalInputTokens", m.total_input_tokens)?;
        set_opt(&metrics, "totalOutputTokens", m.total_output_tokens)?;
        set_opt(&metrics, "contextWindowSize", m.context_window_size)?;
        set_opt(&metrics, "usedPercentage", m.used_percentage)?;
        set_opt(&metrics, "cacheReadInputTokens", m.cache_read_input_tokens)?;
        set_opt(
            &metrics,
            "cacheCreationInputTokens",
            m.cache_creation_input_tokens,
        )?;
        t.set("agentMetrics", metrics)?;
    }

    if let Some(u) = s.usage.as_ref() {
        let usage = lua.create_table()?;
        set_opt(&usage, "plan", u.plan.clone())?;
        set_opt(&usage, "note", u.note.clone())?;
        let windows = lua.create_table()?;
        for (i, w) in u.windows.iter().enumerate() {
            let window = lua.create_table()?;
            window.set("label", w.label.clone())?;
            window.set("usedPercent", w.used_percent)?;
            set_opt(&window, "resetsInSecs", w.resets_in_secs)?;
            windows.set(i + 1, window)?;
        }
        usage.set("windows", windows)?;
        t.set("usage", usage)?;
    }

    Ok(t)
}

/// Host resource metrics, or `Nil` before the first sample.
pub fn metrics_table(lua: &Lua, context: &PaneContext) -> mlua::Result<mlua::Value> {
    let Some(system) = context.system.as_ref() else {
        return Ok(mlua::Value::Nil);
    };
    Ok(mlua::Value::Table(build_system(lua, system)?))
}

fn build_system(lua: &Lua, m: &SystemSnapshot) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("cpuPercent", m.cpu_percent)?;
    t.set("memoryUsed", m.memory_used)?;
    t.set("memoryTotal", m.memory_total)?;
    t.set("sessionCpuPercent", m.session_cpu_percent)?;
    t.set("sessionMemoryBytes", m.session_memory_bytes)?;
    set_opt(&t, "thurboxDirBytes", m.thurbox_dir_bytes)?;
    Ok(t)
}

/// Scheduled automations as an array, soonest first.
///
/// An empty array rather than `Nil`: "there are none" is a fact the kernel knows,
/// unlike "no metrics have been sampled yet", so a plugin can iterate without a
/// nil check.
pub fn automations_table(lua: &Lua, context: &PaneContext) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    for (i, a) in context.automations.iter().enumerate() {
        t.set(i + 1, build_automation(lua, a)?)?;
    }
    Ok(t)
}

fn build_automation(lua: &Lua, a: &AutomationSnapshot) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    // The row's identity, which is what a pane granted `automations-write` passes
    // back when it enables, runs or deletes the automation it drew.
    t.set("id", a.id)?;
    t.set("label", a.label.clone())?;
    t.set("dueInSecs", a.due_in_secs)?;
    Ok(t)
}

/// The task list: `{ entries = { … }, focused = bool }`.
///
/// A table rather than a `Nil`, for the automations reason — "there are no tasks"
/// is knowledge the kernel has, so a pane iterates without a nil check. `entries`
/// is a one-based array so `for _, entry in ipairs(tasks.entries)` works.
pub fn tasks_table(lua: &Lua, context: &PaneContext) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    let entries = lua.create_table()?;
    for (i, task) in context.tasks.entries.iter().enumerate() {
        entries.set(i + 1, build_task(lua, task)?)?;
    }
    t.set("entries", entries)?;
    t.set("focused", context.tasks.focused)?;
    Ok(t)
}

fn build_task(lua: &Lua, task: &TaskSnapshot) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    // As for an automation: the id is how a pane granted `tasks-write` names the
    // row it drew, rather than matching on a title it fitted itself.
    t.set("id", task.id)?;
    t.set("title", task.title.clone())?;
    t.set("status", task.status)?;
    // Booleans cross as booleans even when false: unlike an optional value,
    // "this row is not selected" is a fact every row carries, and a pane reads
    // all three unconditionally.
    t.set("selected", task.selected)?;
    t.set("dimmed", task.dimmed)?;
    t.set("linked", task.linked)?;
    // Byte offsets, not pre-split runs: the split is presentation, so the pane
    // does it. One-based array of the zero-based offsets the matcher produced —
    // the array index is Lua's convention, the values are the kernel's.
    let matches = lua.create_table()?;
    for (i, pos) in task.match_positions.iter().enumerate() {
        matches.set(i + 1, *pos as u64)?;
    }
    t.set("matchPositions", matches)?;
    Ok(t)
}

/// The open file tree: `{ nodes = { … }, selected = n?, nerdFont = bool }`.
///
/// Always a table, for the automations reason — "the viewer has no tree open" is
/// knowledge the kernel has. `nodes` is a one-based array so
/// `for _, node in ipairs(files.nodes)` works, and `selected` is a **one-based**
/// index into it: it is the index a plugin hands straight back to `ui.list`, so
/// the two must agree, and Lua's arrays are one-based.
///
/// `selected` is absent rather than `0` when there is no cursor, which is the same
/// "absent means absent" rule the rest of the boundary uses — and the reason
/// `ui.list` refuses a `0`.
pub fn files_table(lua: &Lua, context: &PaneContext) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    let nodes = lua.create_table()?;
    for (i, node) in context.files.nodes.iter().enumerate() {
        nodes.set(i + 1, build_file_node(lua, node)?)?;
    }
    t.set("nodes", nodes)?;
    set_opt(&t, "selected", context.files.selected.map(|i| i as u64 + 1))?;
    t.set("nerdFont", context.files.nerd_font)?;
    Ok(t)
}

fn build_file_node(lua: &Lua, node: &FileNodeSnapshot) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("name", node.name.clone())?;
    t.set("depth", node.depth as u64)?;
    // Booleans cross even when false: every row carries all three as facts, and
    // a pane reads them unconditionally.
    t.set("isDir", node.is_dir)?;
    t.set("expanded", node.expanded)?;
    t.set("matched", node.matched)?;
    Ok(t)
}

/// The open review's diff stream:
/// `{ lines = { … }, cursor = n?, numberWidth = n }`.
///
/// Always a table, for the file tree's reason — "no review is open" is knowledge
/// the kernel has, and a pane draws its empty state from an empty list rather
/// than from a nil. `cursor` is **one-based**, like `files.selected`, because it
/// is the index a plugin hands straight back to `ui.list`.
///
/// `numberWidth` crosses because it is computed over the whole review, which the
/// published window is not: see
/// [`ReviewSnapshot::number_width`](crate::session::pane_context::ReviewSnapshot::number_width).
pub fn review_table(lua: &Lua, context: &PaneContext) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    let lines = lua.create_table()?;
    for (i, line) in context.review.lines.iter().enumerate() {
        lines.set(i + 1, build_review_line(lua, line)?)?;
    }
    t.set("lines", lines)?;
    set_opt(&t, "cursor", context.review.cursor.map(|i| i as u64 + 1))?;
    t.set("numberWidth", context.review.number_width as u64)?;
    Ok(t)
}

fn build_review_line(lua: &Lua, line: &ReviewLineSnapshot) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("path", line.path.clone())?;
    // Absent rather than zero on the side the line is not on: a deletion has no
    // new-side number, and `0` would be a number a pane could print.
    set_opt(&t, "oldNo", line.old_no.map(u64::from))?;
    set_opt(&t, "newNo", line.new_no.map(u64::from))?;
    t.set("kind", line.kind)?;
    // Raw: not tokenised, not windowed, not padded. Colouring a diff body is the
    // pane's decision, so the text crosses and the highlighting does not.
    t.set("text", line.text.clone())?;
    Ok(t)
}

/// The session list: `{ rows = { … } }`.
///
/// Always a table, for the file tree's reason — "no session is open" is knowledge
/// the kernel has, so a pane draws its empty state from an empty list rather than
/// branching on a nil. `rows` is a one-based array so
/// `for _, row in ipairs(list.rows)` works.
///
/// There is deliberately no top-level cursor index here, unlike `files.selected`
/// and `review.cursor`. A pane's rows are not the list's children one for one —
/// a repo group's first row is preceded by a header row — so the index `ui.list`
/// wants is one the pane knows and the kernel does not. Each row therefore says
/// whether the cursor is on *it*, and the pane counts the children it emitted.
pub fn session_list_table(lua: &Lua, context: &PaneContext) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    let rows = lua.create_table()?;
    for (i, row) in context.session_list.rows.iter().enumerate() {
        rows.set(i + 1, build_session_row(lua, row)?)?;
    }
    t.set("rows", rows)?;
    Ok(t)
}

fn build_session_row(lua: &Lua, row: &SessionRowSnapshot) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("name", row.name.clone())?;

    let status = lua.create_table()?;
    status.set("name", row.status.name)?;
    status.set("label", row.status.label.clone())?;
    status.set("icon", row.status.icon)?;
    status.set("token", row.status.token)?;
    t.set("status", status)?;

    // Absent on every row but a group's first, which is what lets a pane write
    // `if row.group then` and emit a header there.
    set_opt(&t, "group", row.group.clone())?;
    t.set("depth", u64::from(row.depth))?;
    // Booleans cross even when false: every row carries all five as facts and a
    // pane reads them unconditionally.
    t.set("crossGroupChild", row.cross_group_child)?;
    t.set("remote", row.remote)?;
    t.set("worktree", row.worktree)?;
    t.set("selected", row.selected)?;
    t.set("dimmed", row.dimmed)?;

    // Byte offsets, not pre-split runs — the split is presentation. One-based
    // array of the zero-based offsets the matcher produced, exactly as a task
    // row's cross.
    let matches = lua.create_table()?;
    for (i, pos) in row.match_positions.iter().enumerate() {
        matches.set(i + 1, *pos as u64)?;
    }
    t.set("matchPositions", matches)?;

    // Both texts, unresolved: which one a row shows (and whether it shows one at
    // all) is the pane's rule, not the kernel's.
    set_opt(&t, "activity", row.activity.clone())?;
    set_opt(&t, "notification", row.notification.clone())?;
    Ok(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::pane_context::{
        AgentMetricsSnapshot, GitSnapshot, ReviewSnapshot, SessionListSnapshot, StatusSnapshot,
        TasksSnapshot, UsageSnapshot, UsageWindowSnapshot,
    };
    use crate::session::SessionStatus;

    fn session(name: &str) -> SessionSnapshot {
        SessionSnapshot {
            id: "abc".to_string(),
            name: name.to_string(),
            status: StatusSnapshot::of(SessionStatus::Blocked),
            agent: "claude".to_string(),
            parent_name: None,
            remote_host: None,
            hook_wiring: None,
            activity: None,
            notification: None,
            repo_name: Some("thurbox".to_string()),
            branch: Some("main".to_string()),
            additional_dir_names: vec!["docs".to_string(), "web".to_string()],
            git: None,
            agent_metrics: None,
            usage: None,
        }
    }

    fn context(session: Option<SessionSnapshot>) -> PaneContext {
        PaneContext {
            session,
            ..PaneContext::default()
        }
    }

    #[test]
    fn a_session_crosses_with_its_status_triple() {
        let lua = Lua::new();
        let value = session_table(&lua, &context(Some(session("demo")))).unwrap();
        let t = match value {
            mlua::Value::Table(t) => t,
            other => panic!("expected a table, got {other:?}"),
        };
        assert_eq!(t.get::<String>("name").unwrap(), "demo");
        let status: Table = t.get("status").unwrap();
        assert_eq!(status.get::<String>("name").unwrap(), "blocked");
        assert_eq!(status.get::<String>("label").unwrap(), "Blocked");
        assert_eq!(status.get::<String>("icon").unwrap(), "◆");
        assert_eq!(status.get::<String>("token").unwrap(), "status_blocked");
    }

    /// An absent optional is an absent key, so `if session.parentName then`
    /// reads the way a Lua author expects and a sentinel value can never be
    /// mistaken for data.
    #[test]
    fn an_absent_field_is_an_absent_key() {
        let lua = Lua::new();
        let value = session_table(&lua, &context(Some(session("demo")))).unwrap();
        let t = match value {
            mlua::Value::Table(t) => t,
            other => panic!("expected a table, got {other:?}"),
        };
        for key in ["parentName", "remoteHost", "hookWiring", "activity", "git"] {
            assert!(!t.contains_key(key).unwrap(), "{key} should be absent");
            assert!(matches!(
                t.get::<mlua::Value>(key).unwrap(),
                mlua::Value::Nil
            ));
        }
    }

    #[test]
    fn no_active_session_is_nil() {
        let lua = Lua::new();
        assert!(matches!(
            session_table(&lua, &context(None)).unwrap(),
            mlua::Value::Nil
        ));
    }

    #[test]
    fn additional_dirs_cross_as_a_one_based_array() {
        let lua = Lua::new();
        let value = session_table(&lua, &context(Some(session("demo")))).unwrap();
        let t = match value {
            mlua::Value::Table(t) => t,
            other => panic!("expected a table, got {other:?}"),
        };
        let dirs: Table = t.get("additionalDirNames").unwrap();
        assert_eq!(dirs.raw_len(), 2);
        assert_eq!(dirs.get::<String>(1).unwrap(), "docs");
        assert_eq!(dirs.get::<String>(2).unwrap(), "web");
    }

    /// The whole point of publishing raw values: a plugin owns its presentation,
    /// so a byte count must arrive as a number it can format.
    #[test]
    fn quantities_cross_as_numbers_not_strings() {
        let lua = Lua::new();
        let ctx = PaneContext {
            system: Some(SystemSnapshot {
                cpu_percent: 12.5,
                memory_used: 8_589_934_592,
                memory_total: 17_179_869_184,
                session_cpu_percent: 3.25,
                session_memory_bytes: 524_288_000,
                thurbox_dir_bytes: Some(1_048_576),
            }),
            ..PaneContext::default()
        };
        let t = match metrics_table(&lua, &ctx).unwrap() {
            mlua::Value::Table(t) => t,
            other => panic!("expected a table, got {other:?}"),
        };
        assert_eq!(t.get::<u64>("memoryUsed").unwrap(), 8_589_934_592);
        assert_eq!(t.get::<f64>("cpuPercent").unwrap(), 12.5);
        assert_eq!(t.get::<u64>("thurboxDirBytes").unwrap(), 1_048_576);
    }

    #[test]
    fn absent_metrics_are_nil() {
        let lua = Lua::new();
        assert!(matches!(
            metrics_table(&lua, &context(None)).unwrap(),
            mlua::Value::Nil
        ));
    }

    /// Unlike metrics, "no automations" is knowledge rather than absence, so it
    /// crosses as an empty array a plugin can iterate unconditionally.
    #[test]
    fn no_automations_is_an_empty_array() {
        let lua = Lua::new();
        let t = automations_table(&lua, &context(None)).unwrap();
        assert_eq!(t.raw_len(), 0);
    }

    #[test]
    fn automations_carry_a_resolved_countdown() {
        let lua = Lua::new();
        let ctx = PaneContext {
            automations: vec![AutomationSnapshot {
                id: 7,
                label: "nightly".to_string(),
                due_in_secs: 90,
            }],
            ..PaneContext::default()
        };
        let t = automations_table(&lua, &ctx).unwrap();
        let first: Table = t.get(1).unwrap();
        assert_eq!(first.get::<i64>("id").unwrap(), 7);
        assert_eq!(first.get::<String>("label").unwrap(), "nightly");
        assert_eq!(first.get::<u64>("dueInSecs").unwrap(), 90);
    }

    /// A task row carries what a pane draws it from — and no glyph and no style
    /// token, because choosing those is what the pane is for.
    #[test]
    fn a_task_row_crosses_with_its_view_facts_and_no_rendering() {
        let lua = Lua::new();
        let ctx = PaneContext {
            tasks: TasksSnapshot {
                entries: vec![TaskSnapshot {
                    id: 12,
                    title: "ship the pane".to_string(),
                    status: "in_progress",
                    selected: true,
                    dimmed: false,
                    linked: true,
                    match_positions: vec![0, 5],
                }],
                focused: true,
            },
            ..PaneContext::default()
        };
        let t = tasks_table(&lua, &ctx).unwrap();
        assert!(t.get::<bool>("focused").unwrap());
        let entries: Table = t.get("entries").unwrap();
        assert_eq!(entries.raw_len(), 1);
        let row: Table = entries.get(1).unwrap();
        // The id is the row's identity, which a pane that may change a task needs.
        assert_eq!(row.get::<i64>("id").unwrap(), 12);
        assert_eq!(row.get::<String>("title").unwrap(), "ship the pane");
        assert_eq!(row.get::<String>("status").unwrap(), "in_progress");
        assert!(row.get::<bool>("selected").unwrap());
        assert!(!row.get::<bool>("dimmed").unwrap());
        assert!(row.get::<bool>("linked").unwrap());
        let matches: Table = row.get("matchPositions").unwrap();
        assert_eq!(matches.get::<u64>(1).unwrap(), 0);
        assert_eq!(matches.get::<u64>(2).unwrap(), 5);
        for absent in ["icon", "glyph", "token", "style"] {
            assert!(
                !row.contains_key(absent).unwrap(),
                "a task row must not carry its own rendering ({absent})"
            );
        }
    }

    /// Unlike an optional field, a row's flags are facts every row has, so
    /// `false` must cross as `false` rather than as an absent key — a pane reads
    /// all three unconditionally.
    #[test]
    fn a_task_rows_flags_cross_even_when_false() {
        let lua = Lua::new();
        let ctx = PaneContext {
            tasks: TasksSnapshot {
                entries: vec![TaskSnapshot {
                    id: 1,
                    title: "t".to_string(),
                    status: "todo",
                    selected: false,
                    dimmed: false,
                    linked: false,
                    match_positions: Vec::new(),
                }],
                focused: false,
            },
            ..PaneContext::default()
        };
        let entries: Table = tasks_table(&lua, &ctx).unwrap().get("entries").unwrap();
        let row: Table = entries.get(1).unwrap();
        for key in ["selected", "dimmed", "linked"] {
            assert!(row.contains_key(key).unwrap(), "{key} should be present");
            assert!(!row.get::<bool>(key).unwrap());
        }
        let matches: Table = row.get("matchPositions").unwrap();
        assert_eq!(matches.raw_len(), 0);
    }

    /// "There are no tasks" is knowledge, like "there are no automations", so it
    /// crosses as an empty array inside a present table.
    #[test]
    fn no_tasks_is_an_empty_array() {
        let lua = Lua::new();
        let t = tasks_table(&lua, &context(None)).unwrap();
        let entries: Table = t.get("entries").unwrap();
        assert_eq!(entries.raw_len(), 0);
        assert!(!t.get::<bool>("focused").unwrap());
    }

    // ── the session list ──

    fn session_row(name: &str, status: SessionStatus) -> SessionRowSnapshot {
        SessionRowSnapshot {
            name: name.to_string(),
            status: StatusSnapshot::of(status),
            group: None,
            depth: 0,
            cross_group_child: false,
            remote: false,
            worktree: false,
            selected: false,
            dimmed: false,
            match_positions: Vec::new(),
            activity: None,
            notification: None,
        }
    }

    fn session_list_context(rows: Vec<SessionRowSnapshot>) -> PaneContext {
        PaneContext {
            session_list: SessionListSnapshot { rows },
            ..PaneContext::default()
        }
    }

    #[test]
    fn a_session_row_crosses_with_its_view_facts_and_no_rendering() {
        let lua = Lua::new();
        let ctx = session_list_context(vec![SessionRowSnapshot {
            group: Some("thurbox".to_string()),
            depth: 2,
            cross_group_child: true,
            remote: true,
            worktree: true,
            selected: true,
            dimmed: true,
            match_positions: vec![0, 4],
            activity: Some("Compacting".to_string()),
            notification: Some("Approve?".to_string()),
            ..session_row("worker", SessionStatus::Working)
        }]);

        let table = session_list_table(&lua, &ctx).unwrap();
        let rows: Table = table.get("rows").unwrap();
        assert_eq!(rows.raw_len(), 1);
        let row: Table = rows.get(1).unwrap();

        assert_eq!(row.get::<String>("name").unwrap(), "worker");
        assert_eq!(row.get::<String>("group").unwrap(), "thurbox");
        assert_eq!(row.get::<u64>("depth").unwrap(), 2);
        assert!(row.get::<bool>("crossGroupChild").unwrap());
        assert!(row.get::<bool>("remote").unwrap());
        assert!(row.get::<bool>("worktree").unwrap());
        assert!(row.get::<bool>("selected").unwrap());
        assert!(row.get::<bool>("dimmed").unwrap());
        assert_eq!(row.get::<String>("activity").unwrap(), "Compacting");
        assert_eq!(row.get::<String>("notification").unwrap(), "Approve?");

        // The status triple, which is the one *rendering* that crosses — a pane
        // must not re-derive which colour a state gets.
        let status: Table = row.get("status").unwrap();
        assert_eq!(status.get::<String>("name").unwrap(), "working");
        assert_eq!(status.get::<String>("token").unwrap(), "status_working");
        assert!(!status.get::<String>("icon").unwrap().is_empty());

        // Offsets, not runs: the split into emphasised and plain is presentation.
        let matches: Table = row.get("matchPositions").unwrap();
        assert_eq!(matches.raw_len(), 2);
        assert_eq!(matches.get::<u64>(1).unwrap(), 0);
        assert_eq!(matches.get::<u64>(2).unwrap(), 4);

        // No composed row, no prefix, no fitted text, and no cursor index on the
        // list: a group header makes rows and list children differ, so the index
        // is the pane's to count.
        for absent in ["line", "prefix", "text", "glyph"] {
            assert!(row.get::<mlua::Value>(absent).unwrap().is_nil());
        }
        assert!(table.get::<mlua::Value>("cursor").unwrap().is_nil());
        assert!(table.get::<mlua::Value>("selected").unwrap().is_nil());
    }

    #[test]
    fn a_session_rows_flags_cross_even_when_false() {
        let lua = Lua::new();
        let ctx = session_list_context(vec![session_row("plain", SessionStatus::Idle)]);
        let rows: Table = session_list_table(&lua, &ctx).unwrap().get("rows").unwrap();
        let row: Table = rows.get(1).unwrap();
        // A fact every row carries is a `false`, not a missing key — a pane reads
        // all five unconditionally.
        for flag in [
            "crossGroupChild",
            "remote",
            "worktree",
            "selected",
            "dimmed",
        ] {
            assert!(!row.get::<bool>(flag).unwrap(), "{flag}");
        }
        // An absent value is an absent key, which is the other half of the rule.
        for absent in ["group", "activity", "notification"] {
            assert!(row.get::<mlua::Value>(absent).unwrap().is_nil(), "{absent}");
        }
        // And an empty match list is still a table, so `ipairs` needs no guard.
        assert_eq!(row.get::<Table>("matchPositions").unwrap().raw_len(), 0);
    }

    #[test]
    fn no_sessions_is_an_empty_array() {
        let lua = Lua::new();
        let table = session_list_table(&lua, &PaneContext::default()).unwrap();
        assert_eq!(table.get::<Table>("rows").unwrap().raw_len(), 0);
    }

    // ── the open file tree ──

    fn file_context(nodes: Vec<FileNodeSnapshot>, selected: Option<usize>) -> PaneContext {
        PaneContext {
            files: crate::session::pane_context::FilesSnapshot {
                nodes,
                selected,
                nerd_font: true,
            },
            ..PaneContext::default()
        }
    }

    fn node(name: &str, depth: usize, is_dir: bool) -> FileNodeSnapshot {
        FileNodeSnapshot {
            name: name.to_string(),
            depth,
            is_dir,
            expanded: is_dir,
            matched: true,
        }
    }

    #[test]
    fn a_file_row_crosses_with_its_view_facts_and_no_path() {
        let lua = Lua::new();
        let ctx = file_context(
            vec![node("src", 0, true), node("main.rs", 1, false)],
            Some(1),
        );
        let files = files_table(&lua, &ctx).unwrap();
        let nodes: Table = files.get("nodes").unwrap();
        assert_eq!(nodes.raw_len(), 2);

        let root: Table = nodes.get(1).unwrap();
        assert_eq!(root.get::<String>("name").unwrap(), "src");
        assert_eq!(root.get::<u64>("depth").unwrap(), 0);
        assert!(root.get::<bool>("isDir").unwrap());
        assert!(root.get::<bool>("expanded").unwrap());
        assert!(root.get::<bool>("matched").unwrap());
        // Nothing that locates the row on disk, and no rendering of it.
        for absent in ["path", "root", "glyph", "marker", "style"] {
            assert!(
                !root.contains_key(absent).unwrap(),
                "a row must not carry `{absent}`"
            );
        }

        // The cursor crosses one-based, because it is the index a plugin hands
        // straight to `ui.list`.
        assert_eq!(files.get::<u64>("selected").unwrap(), 2);
        assert!(files.get::<bool>("nerdFont").unwrap());
    }

    #[test]
    fn a_falsey_file_flag_still_crosses() {
        let lua = Lua::new();
        let mut row = node("hidden-by-search", 1, false);
        row.matched = false;
        let files = files_table(&lua, &file_context(vec![row], Some(0))).unwrap();
        let first: Table = files.get::<Table>("nodes").unwrap().get(1).unwrap();
        assert!(!first.get::<bool>("matched").unwrap());
        assert!(!first.get::<bool>("isDir").unwrap());
        assert!(!first.get::<bool>("expanded").unwrap());
    }

    /// No cursor is an **absent** key, not a `0`: the same rule the rest of the
    /// boundary uses, and the reason `ui.list` refuses a zero index.
    #[test]
    fn an_empty_tree_is_an_empty_array_with_no_cursor() {
        let lua = Lua::new();
        let files = files_table(&lua, &PaneContext::default()).unwrap();
        assert_eq!(files.get::<Table>("nodes").unwrap().raw_len(), 0);
        assert!(matches!(
            files.get::<mlua::Value>("selected").unwrap(),
            mlua::Value::Nil
        ));
        assert!(!files.get::<bool>("nerdFont").unwrap());
    }

    #[test]
    fn nested_sections_cross_whole() {
        let lua = Lua::new();
        let mut s = session("demo");
        s.git = Some(GitSnapshot {
            files_changed: 3,
            insertions: 40,
            deletions: 7,
            dirty: true,
            ahead: 2,
            behind: 1,
        });
        s.agent_metrics = Some(AgentMetricsSnapshot {
            model_display_name: Some("Opus".to_string()),
            total_input_tokens: Some(1_500),
            ..AgentMetricsSnapshot::default()
        });
        s.usage = Some(UsageSnapshot {
            windows: vec![UsageWindowSnapshot {
                label: "5h".to_string(),
                used_percent: 42.0,
                resets_in_secs: Some(3_600),
            }],
            plan: Some("max".to_string()),
            note: None,
        });
        let value = session_table(&lua, &context(Some(s))).unwrap();
        let t = match value {
            mlua::Value::Table(t) => t,
            other => panic!("expected a table, got {other:?}"),
        };
        let git: Table = t.get("git").unwrap();
        assert_eq!(git.get::<u64>("insertions").unwrap(), 40);
        assert!(git.get::<bool>("dirty").unwrap());
        let metrics: Table = t.get("agentMetrics").unwrap();
        assert_eq!(metrics.get::<String>("modelDisplayName").unwrap(), "Opus");
        assert!(
            !metrics.contains_key("totalCostUsd").unwrap(),
            "an unreported metric must not arrive as zero"
        );
        let usage: Table = t.get("usage").unwrap();
        assert_eq!(usage.get::<String>("plan").unwrap(), "max");
        let windows: Table = usage.get("windows").unwrap();
        let window: Table = windows.get(1).unwrap();
        assert_eq!(window.get::<u64>("resetsInSecs").unwrap(), 3_600);
    }

    fn review_context(lines: Vec<ReviewLineSnapshot>, cursor: Option<usize>) -> PaneContext {
        PaneContext {
            review: ReviewSnapshot {
                lines,
                cursor,
                number_width: 4,
            },
            ..PaneContext::default()
        }
    }

    fn review_line(
        kind: &'static str,
        old_no: Option<u32>,
        new_no: Option<u32>,
    ) -> ReviewLineSnapshot {
        ReviewLineSnapshot {
            path: "src/a.rs".to_string(),
            old_no,
            new_no,
            kind,
            text: "let x = 1;".to_string(),
        }
    }

    /// A published diff crosses as a one-based array whose entries carry both
    /// line numbers where they exist and neither where they do not.
    #[test]
    fn a_review_line_crosses_with_the_numbers_it_has() {
        let lua = Lua::new();
        let context = review_context(
            vec![
                review_line("add", None, Some(12)),
                review_line("del", Some(11), None),
                review_line("context", Some(12), Some(13)),
            ],
            Some(1),
        );
        let review = review_table(&lua, &context).unwrap();
        let lines: Table = review.get("lines").unwrap();
        assert_eq!(lines.raw_len(), 3);

        let added: Table = lines.get(1).unwrap();
        assert_eq!(added.get::<String>("kind").unwrap(), "add");
        assert_eq!(added.get::<u64>("newNo").unwrap(), 12);
        assert_eq!(
            added.get::<mlua::Value>("oldNo").unwrap(),
            mlua::Value::Nil,
            "an insertion has no old-side number, and absent means absent"
        );
        assert_eq!(added.get::<String>("path").unwrap(), "src/a.rs");
        assert_eq!(added.get::<String>("text").unwrap(), "let x = 1;");

        let deleted: Table = lines.get(2).unwrap();
        assert_eq!(deleted.get::<u64>("oldNo").unwrap(), 11);
        assert_eq!(
            deleted.get::<mlua::Value>("newNo").unwrap(),
            mlua::Value::Nil
        );

        // One-based, because it is the index the plugin hands to `ui.list`.
        assert_eq!(review.get::<u64>("cursor").unwrap(), 2);
        assert_eq!(review.get::<u64>("numberWidth").unwrap(), 4);
    }

    /// No review open is a present, empty section — the pane draws its own empty
    /// state rather than branching on a nil.
    #[test]
    fn an_empty_review_is_still_a_table() {
        let lua = Lua::new();
        let review = review_table(&lua, &PaneContext::default()).unwrap();
        assert_eq!(review.get::<Table>("lines").unwrap().raw_len(), 0);
        assert_eq!(
            review.get::<mlua::Value>("cursor").unwrap(),
            mlua::Value::Nil
        );
        assert_eq!(review.get::<u64>("numberWidth").unwrap(), 0);
    }
}
