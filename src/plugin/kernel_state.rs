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
    AutomationSnapshot, PaneContext, SessionSnapshot, SystemSnapshot,
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
    t.set("label", a.label.clone())?;
    t.set("dueInSecs", a.due_in_secs)?;
    Ok(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::pane_context::{
        AgentMetricsSnapshot, GitSnapshot, StatusSnapshot, UsageSnapshot, UsageWindowSnapshot,
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
            system: None,
            automations: Vec::new(),
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
            session: None,
            system: Some(SystemSnapshot {
                cpu_percent: 12.5,
                memory_used: 8_589_934_592,
                memory_total: 17_179_869_184,
                session_cpu_percent: 3.25,
                session_memory_bytes: 524_288_000,
                thurbox_dir_bytes: Some(1_048_576),
            }),
            automations: Vec::new(),
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
            session: None,
            system: None,
            automations: vec![AutomationSnapshot {
                label: "nightly".to_string(),
                due_in_secs: 90,
            }],
        };
        let t = automations_table(&lua, &ctx).unwrap();
        let first: Table = t.get(1).unwrap();
        assert_eq!(first.get::<String>("label").unwrap(), "nightly");
        assert_eq!(first.get::<u64>("dueInSecs").unwrap(), 90);
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
}
