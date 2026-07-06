//! `thurbox-cli doctor` — aggregate runtime-environment health check.
//!
//! `config validate` parses the config *files*; `doctor` probes the *runtime*:
//! is `tmux` (>= 3.2) present, is `git` present, is each agent's `command` on
//! PATH, does the database open writable, and (rolled up, not re-implemented)
//! are the config files valid. With `--hosts` it also probes each configured /
//! discovered host for reachability. It is the local counterpart of the e2e
//! harnesses' `check` verb — a readiness probe you can run before ever
//! launching a session.
//!
//! Doctor *reports*; it never installs tmux, seeds config, or edits PATH. It
//! exits non-zero when any **error**-severity check fails (a usable CI/setup
//! gate), zero when only warnings are present. Notification delivery is *not*
//! re-probed here — that is `thurbox-cli notify`'s job; doctor emits a single
//! advisory line pointing at it.
//!
//! The agent module's helpers (`TmuxBackend`, `host_config`, `agent_config`)
//! are reached via fully-qualified paths (no `use crate::agent`) to keep the
//! cli module free of an `agent` import — see
//! tests/architecture_rules.rs::cli_module_isolation.

use std::sync::mpsc;
use std::time::Duration;

use clap::Args;
use serde_json::{json, Value};

use crate::storage::Database;

use super::output::CommandOutput;

/// Per-host reachability probe timeout. A dead box (or an SSH auth prompt)
/// must not hang the whole command; host probes run concurrently, so wall time
/// is bounded to roughly this regardless of host count.
const HOST_PROBE_TIMEOUT: Duration = Duration::from_secs(10);

/// `doctor` subcommand arguments.
#[derive(Args, Debug)]
pub struct DoctorArgs {
    /// Also probe each configured/discovered host for reachability (SSH round
    /// trip per host; off by default because it is slow and non-hermetic).
    #[arg(long)]
    pub hosts: bool,
}

/// Severity of a single check outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    /// Healthy.
    Ok,
    /// Degraded but non-fatal (e.g. a non-default agent's binary is absent).
    Warn,
    /// A problem that fails the command's exit code.
    Error,
}

impl Status {
    /// JSON/string form.
    fn as_str(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Warn => "warn",
            Status::Error => "error",
        }
    }

    /// Leading glyph in the human rendering (mirrors `config validate`).
    fn mark(self) -> &'static str {
        match self {
            Status::Ok => "✓",
            Status::Warn => "·",
            Status::Error => "✗",
        }
    }
}

/// One environment check's outcome.
#[derive(Debug)]
struct Check {
    /// Stable identifier (`tmux`, `git`, `agent:claude`, `database`, `config`,
    /// `host:ssh:devbox`).
    id: String,
    status: Status,
    /// Short human-readable result (`3.4  ok`, `not found on PATH`, …).
    detail: String,
    /// Optional remediation pointer.
    hint: Option<String>,
}

impl Check {
    fn ok(id: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            status: Status::Ok,
            detail: detail.into(),
            hint: None,
        }
    }

    fn new(
        id: impl Into<String>,
        status: Status,
        detail: impl Into<String>,
        hint: Option<String>,
    ) -> Self {
        Self {
            id: id.into(),
            status,
            detail: detail.into(),
            hint,
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "status": self.status.as_str(),
            "detail": self.detail,
            "hint": self.hint,
        })
    }
}

/// Run the `doctor` command. Reads the effective config (agents/hosts) and the
/// already-open database; probes the local environment and, with `--hosts`, the
/// configured host set.
pub fn run(args: DoctorArgs, db: &Database) -> CommandOutput {
    let mut checks = vec![check_tmux(), check_git()];
    let agents = crate::agent::agent_config::load_or_seed();
    checks.extend(agent_checks(&agents));
    checks.push(check_database(db));
    checks.push(check_config());
    if args.hosts {
        checks.extend(host_checks());
    }

    let healthy = healthy(&checks);
    let headline = headline(&checks);
    let advisory = notification_advisory();
    let human = render(&checks, &headline, advisory.as_deref());

    let report = json!({
        "healthy": healthy,
        "checks": checks.iter().map(Check::to_json).collect::<Vec<_>>(),
        "advisory": advisory,
        "summary": headline,
    });

    if healthy {
        CommandOutput::new(report, human)
    } else {
        // Exit non-zero so `doctor` is usable as a CI/setup gate.
        CommandOutput::failed(report, human, headline)
    }
}

/// `tmux` (>= 3.2) present over the local transport.
fn check_tmux() -> Check {
    match crate::agent::tmux::TmuxBackend::local().probe() {
        Ok(version) => Check::ok("tmux", format!("{version}  ok")),
        Err(e) => Check::new(
            "tmux",
            Status::Error,
            e.to_string(),
            Some("install tmux >= 3.2 (thurbox's session backend requires it)".into()),
        ),
    }
}

/// `git` present (worktree creation shells out to it).
fn check_git() -> Check {
    // The cli module may not reference the `git` module (architecture rule), so
    // probe the binary directly — a bare `git --version` needs no repo context.
    match std::process::Command::new("git").arg("--version").output() {
        Ok(out) if out.status.success() => {
            let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
            Check::ok("git", format!("{v}  ok"))
        }
        Ok(out) => Check::new(
            "git",
            Status::Error,
            format!(
                "git --version failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ),
            Some("install git".into()),
        ),
        Err(_) => Check::new(
            "git",
            Status::Error,
            "not found on PATH",
            Some("install git (thurbox creates worktrees with it)".into()),
        ),
    }
}

/// One `agent:<name>` check per registered agent: its `command` resolved on the
/// local PATH. The default agent's absence is an **error** (the common first-run
/// wall); a non-default agent's absence a **warning**. Remote-host agent
/// presence isn't verified here (PATH differs per host).
fn agent_checks(agents: &crate::session::AgentRegistry) -> Vec<Check> {
    let default = agents.default_name();
    agents
        .agents
        .iter()
        .map(|a| {
            let id = format!("agent:{}", a.name);
            if crate::paths::which_on_path(&a.command) {
                Check::ok(id, format!("{}  found", a.command))
            } else if a.name == default {
                Check::new(
                    id,
                    Status::Error,
                    format!("{}: not found on PATH", a.command),
                    Some(format!(
                        "install `{}` or set a different `default` agent in agents.toml",
                        a.command
                    )),
                )
            } else {
                Check::new(
                    id,
                    Status::Warn,
                    format!("{}: not found on PATH (non-default)", a.command),
                    None,
                )
            }
        })
        .collect()
}

/// The database opens writable and isn't held by another writer.
fn check_database(db: &Database) -> Check {
    match db.check_writable() {
        Ok(()) => Check::ok("database", "writable  ok"),
        Err(e) => Check::new(
            "database",
            Status::Error,
            format!("not writable: {e}"),
            Some("check filesystem permissions / free space for the thurbox data dir".into()),
        ),
    }
}

/// Roll up `config validate` as one check (delegated, never re-parsed).
fn check_config() -> Check {
    let (_, failed) = super::config::validate();
    if failed.is_empty() {
        Check::ok("config", "all files valid")
    } else {
        Check::new(
            "config",
            Status::Error,
            format!("invalid: {}", failed.join(", ")),
            Some("run `thurbox-cli config validate` for the per-file details".into()),
        )
    }
}

/// Probe every effective host (configured SSH/WSL + discovered WSL distros) for
/// reachability, concurrently. Each host's `tmux -V` over its transport verifies
/// connectivity + tmux presence + version in one shot; an unreachable or
/// too-old host is an **error**. Only run under `--hosts`.
fn host_checks() -> Vec<Check> {
    let hosts = crate::agent::host_config::load_all();
    if hosts.hosts.is_empty() {
        return vec![Check::ok("hosts", "none configured")];
    }

    // Spawn one probe thread per host so wall time is bounded to the slowest
    // (mirrors main.rs's background remote-restore), each reporting on its own
    // channel we wait on with a timeout.
    let probes: Vec<(String, mpsc::Receiver<Result<String, String>>)> = hosts
        .hosts
        .iter()
        .cloned()
        .map(|host| {
            let backend_name = host.backend_name();
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let result = crate::agent::tmux::TmuxBackend::from_host(&host)
                    .probe()
                    .map_err(|e| e.to_string());
                // Receiver may be gone if the caller already timed out; ignore.
                let _ = tx.send(result);
            });
            (format!("host:{backend_name}"), rx)
        })
        .collect();

    probes
        .into_iter()
        .map(|(id, rx)| match rx.recv_timeout(HOST_PROBE_TIMEOUT) {
            Ok(Ok(version)) => Check::ok(id, format!("reachable  {version}")),
            Ok(Err(e)) => Check::new(id, Status::Error, e, None),
            Err(_) => Check::new(
                id,
                Status::Error,
                format!("timed out after {}s", HOST_PROBE_TIMEOUT.as_secs()),
                None,
            ),
        })
        .collect()
}

/// A single advisory line pointing at `thurbox-cli notify` when notifications
/// are enabled — doctor stays hands-off and never re-probes the delivery
/// backend. `None` when the feature is off (nothing to advise).
fn notification_advisory() -> Option<String> {
    if crate::session::settings::global().features.notifications {
        Some("Notifications are enabled — run `thurbox-cli notify` to diagnose delivery.".into())
    } else {
        None
    }
}

/// Whether the environment is healthy: no **error**-severity checks. Warnings
/// alone stay healthy (e.g. a non-default agent absent).
fn healthy(checks: &[Check]) -> bool {
    !checks.iter().any(|c| c.status == Status::Error)
}

/// One actionable summary line tailored to the outcome (mirrors notify's
/// `diagnose_headline`).
fn headline(checks: &[Check]) -> String {
    let errors: Vec<&str> = checks
        .iter()
        .filter(|c| c.status == Status::Error)
        .map(|c| c.id.as_str())
        .collect();
    let warns = checks.iter().filter(|c| c.status == Status::Warn).count();
    if !errors.is_empty() {
        return format!("{} problem(s): {}.", errors.len(), errors.join(", "));
    }
    if warns > 0 {
        return format!("Environment healthy ({warns} warning(s)).");
    }
    "Environment healthy.".to_string()
}

/// Render the per-check status list plus headline and optional advisory,
/// modeled on `config validate`'s `render_validate`.
fn render(checks: &[Check], headline: &str, advisory: Option<&str>) -> String {
    let mut lines = Vec::new();
    for c in checks {
        lines.push(format!("{} {}  {}", c.status.mark(), c.id, c.detail));
        if let Some(hint) = &c.hint {
            lines.push(format!("    → {hint}"));
        }
    }
    lines.push(String::new());
    lines.push(headline.to_string());
    if let Some(a) = advisory {
        lines.push(a.to_string());
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::TestPathGuard;

    fn err(id: &str) -> Check {
        Check::new(id, Status::Error, "boom", None)
    }
    fn warn(id: &str) -> Check {
        Check::new(id, Status::Warn, "meh", None)
    }

    #[test]
    fn healthy_only_when_no_errors() {
        assert!(healthy(&[Check::ok("a", "ok"), warn("b")]));
        assert!(!healthy(&[Check::ok("a", "ok"), err("b")]));
        assert!(healthy(&[]));
    }

    #[test]
    fn headline_names_error_checks() {
        let h = headline(&[Check::ok("a", "ok"), err("tmux"), err("git")]);
        assert!(h.contains("2 problem(s)"), "got: {h}");
        assert!(h.contains("tmux") && h.contains("git"), "got: {h}");
    }

    #[test]
    fn headline_counts_warnings_when_otherwise_healthy() {
        let h = headline(&[Check::ok("a", "ok"), warn("agent:codex")]);
        assert!(h.contains("healthy"), "got: {h}");
        assert!(h.contains("1 warning"), "got: {h}");
    }

    #[test]
    fn headline_is_plain_when_all_ok() {
        assert_eq!(headline(&[Check::ok("a", "ok")]), "Environment healthy.");
        // Empty is vacuously healthy too (no checks ran).
        assert_eq!(headline(&[]), "Environment healthy.");
    }

    #[test]
    fn status_string_and_glyph_mapping() {
        assert_eq!(Status::Ok.as_str(), "ok");
        assert_eq!(Status::Warn.as_str(), "warn");
        assert_eq!(Status::Error.as_str(), "error");
        assert_eq!(Status::Ok.mark(), "✓");
        assert_eq!(Status::Warn.mark(), "·");
        assert_eq!(Status::Error.mark(), "✗");
    }

    #[test]
    fn render_lists_checks_marks_and_hints() {
        let checks = vec![
            Check::ok("tmux", "3.4  ok"),
            Check::new(
                "git",
                Status::Error,
                "not found on PATH",
                Some("install git".into()),
            ),
        ];
        let out = render(&checks, "1 problem(s): git.", Some("advisory line"));
        assert!(out.contains("✓ tmux  3.4  ok"), "got:\n{out}");
        assert!(out.contains("✗ git  not found on PATH"), "got:\n{out}");
        assert!(out.contains("    → install git"), "got:\n{out}");
        assert!(out.contains("1 problem(s): git."), "got:\n{out}");
        assert!(out.contains("advisory line"), "got:\n{out}");
    }

    #[test]
    fn render_without_advisory_omits_the_line() {
        let out = render(
            &[Check::ok("tmux", "3.4  ok")],
            "Environment healthy.",
            None,
        );
        assert!(out.ends_with("Environment healthy."), "got:\n{out}");
        assert!(!out.contains("Notifications"), "got:\n{out}");
    }

    #[test]
    fn notification_advisory_points_at_notify_when_enabled() {
        // The default test-process settings have notifications enabled, so the
        // advisory is present and points the user at `thurbox-cli notify`.
        let a = notification_advisory().expect("advisory when notifications on");
        assert!(a.contains("notify"), "got: {a}");
    }

    #[test]
    fn agent_checks_default_absence_is_error_others_warn() {
        // Two agents with commands that cannot exist on PATH; the default's
        // absence is an error, the non-default's a warning.
        let agent = |name: &str, command: &str| crate::session::AgentDef {
            name: name.into(),
            command: command.into(),
            args: vec![],
            resume_args: vec![],
            fork_args: vec![],
            new_session_args: vec![],
            resume_latest: false,
        };
        let reg = crate::session::AgentRegistry {
            config_version: None,
            default: "primary".into(),
            agents: vec![
                agent("primary", "thurbox-doctor-missing-primary"),
                agent("secondary", "thurbox-doctor-missing-secondary"),
            ],
        };
        let checks = agent_checks(&reg);
        assert_eq!(checks[0].id, "agent:primary");
        assert_eq!(checks[0].status, Status::Error);
        assert_eq!(checks[1].id, "agent:secondary");
        assert_eq!(checks[1].status, Status::Warn);
    }

    #[test]
    fn config_check_ok_on_fresh_environment() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = TestPathGuard::new(tmp.path());
        let c = check_config();
        assert_eq!(c.status, Status::Ok, "detail: {}", c.detail);
    }

    #[test]
    fn config_check_errors_on_malformed_file() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = TestPathGuard::new(tmp.path());
        let path = crate::agent::agent_config::agents_config_path().unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "not toml {{{").unwrap();

        let c = check_config();
        assert_eq!(c.status, Status::Error);
        assert!(c.detail.contains("agents.toml"), "got: {}", c.detail);
    }

    #[test]
    fn database_check_ok_on_open_db() {
        let db = Database::open_in_memory().unwrap();
        assert_eq!(check_database(&db).status, Status::Ok);
    }

    #[test]
    fn run_reports_expected_check_ids_and_shape() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = TestPathGuard::new(tmp.path());
        let db = Database::open_in_memory().unwrap();

        // No --hosts: local checks only. Assert the report *shape* and that each
        // fixed check id is present — not the concrete tmux/git pass, which
        // depends on the host (may be absent on some CI runners).
        let out = run(DoctorArgs { hosts: false }, &db);
        assert!(out["healthy"].is_boolean());
        let checks = out["checks"].as_array().expect("checks array");
        assert!(!checks.is_empty());
        for c in checks {
            assert!(c["id"].is_string());
            let status = c["status"].as_str().unwrap();
            assert!(
                matches!(status, "ok" | "warn" | "error"),
                "unexpected status: {status}"
            );
            assert!(c["detail"].is_string());
        }
        let ids: Vec<&str> = checks.iter().filter_map(|c| c["id"].as_str()).collect();
        for id in ["tmux", "git", "database", "config"] {
            assert!(ids.contains(&id), "missing check `{id}` in {ids:?}");
        }
        // The default agent (claude) yields an `agent:claude` row.
        assert!(
            ids.iter().any(|id| id.starts_with("agent:")),
            "expected at least one agent check in {ids:?}"
        );
        // No hosts configured in a fresh env, so no host:* rows appeared.
        assert!(!ids.iter().any(|id| id.starts_with("host:")));
    }
}
