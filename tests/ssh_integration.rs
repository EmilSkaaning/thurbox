//! End-to-end integration test for thurbox's **remote SSH session** path.
//!
//! This exercises the *real* code — `git::*_on` over `ssh <dest> git …` and
//! `TmuxBackend::from_host` driving tmux control mode over `ssh <dest> tmux …`
//! — against a live sshd. It is **opt-in** and **skipped by default** so the
//! normal `cargo nextest run` stays hermetic: it runs only when
//! `THURBOX_SSH_IT=1` is set.
//!
//! ## Running it
//!
//! ```sh
//! ./scripts/test-ssh/up.sh                 # sshd + tmux + git in a container
//! THURBOX_SSH_IT=1 cargo nextest run --test ssh_integration --no-capture
//! ./scripts/test-ssh/down.sh
//! ```
//!
//! The container (see `scripts/test-ssh/`) listens on `127.0.0.1:2222`, accepts
//! a generated key, and ships a pre-seeded git repo at `/home/tester/repo`
//! (branch `main`) plus a worktrees dir at `/home/tester/worktrees`. The env
//! vars below default to that container, so `THURBOX_SSH_IT=1` alone is enough.
//!
//! | Env var               | Default              | Meaning                          |
//! |-----------------------|----------------------|----------------------------------|
//! | `THURBOX_SSH_IT`      | (unset → skip)       | gate; must be `1` to run         |
//! | `THURBOX_SSH_DEST`    | `tester@localhost`   | ssh destination                  |
//! | `THURBOX_SSH_PORT`    | `2222`               | ssh port                         |
//! | `THURBOX_SSH_KEY`     | `scripts/test-ssh/.keys/id_ed25519` | identity file     |
//! | `THURBOX_SSH_REPO`    | `/home/tester/repo`  | remote git repo                  |
//! | `THURBOX_SSH_WORKTREES` | `/home/tester/worktrees` | remote worktrees dir       |

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime};

use thurbox::agent::tmux::TmuxBackend;
use thurbox::agent::{GenericProvider, Session, SessionBackend};
use thurbox::git;
use thurbox::session::{AgentDef, HostDef, SessionConfig};

/// Read the integration-test configuration from the environment, returning
/// `None` (which means *skip*) unless `THURBOX_SSH_IT=1`.
fn config_from_env() -> Option<ItConfig> {
    if std::env::var("THURBOX_SSH_IT").ok().as_deref() != Some("1") {
        return None;
    }
    let key = std::env::var("THURBOX_SSH_KEY").unwrap_or_else(|_| {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("scripts/test-ssh/.keys/id_ed25519")
            .display()
            .to_string()
    });
    Some(ItConfig {
        destination: std::env::var("THURBOX_SSH_DEST")
            .unwrap_or_else(|_| "tester@localhost".to_string()),
        port: std::env::var("THURBOX_SSH_PORT").unwrap_or_else(|_| "2222".to_string()),
        key,
        repo: std::env::var("THURBOX_SSH_REPO").unwrap_or_else(|_| "/home/tester/repo".to_string()),
        worktrees: std::env::var("THURBOX_SSH_WORKTREES")
            .unwrap_or_else(|_| "/home/tester/worktrees".to_string()),
    })
}

struct ItConfig {
    destination: String,
    port: String,
    key: String,
    repo: String,
    worktrees: String,
}

impl ItConfig {
    /// Non-interactive ssh options pinned for a throwaway localhost container:
    /// explicit port + key, no host-key prompts, and a multiplexed master so
    /// the many short ssh calls share one TCP/auth round-trip.
    fn ssh_opts(&self) -> Vec<String> {
        vec![
            "-p".into(),
            self.port.clone(),
            "-i".into(),
            self.key.clone(),
            "-o".into(),
            "StrictHostKeyChecking=no".into(),
            "-o".into(),
            "UserKnownHostsFile=/dev/null".into(),
            "-o".into(),
            "BatchMode=yes".into(),
            "-o".into(),
            "ControlMaster=auto".into(),
            "-o".into(),
            "ControlPersist=10m".into(),
        ]
    }

    fn host(&self) -> HostDef {
        HostDef {
            name: "test-docker".into(),
            destination: self.destination.clone(),
            socket: None,
            session: None,
            ssh_opts: self.ssh_opts(),
            // Pin the worktrees dir so we don't depend on remote $HOME resolution.
            worktrees_dir: Some(self.worktrees.clone()),
        }
    }
}

/// Run a raw `ssh … <cmd>` and return stdout, asserting success. Used only for
/// test assertions/cleanup, never for the code under test.
fn ssh_exec(cfg: &ItConfig, remote_cmd: &str) -> String {
    let output = Command::new("ssh")
        .args(cfg.ssh_opts())
        .arg(&cfg.destination)
        .arg(remote_cmd)
        .output()
        .expect("spawn ssh");
    assert!(
        output.status.success(),
        "remote `{remote_cmd}` failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// A process-unique, slash-free branch/marker suffix (no external rng needed).
fn unique_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{}-{nanos}", std::process::id())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_ssh_full_path_worktree_then_tmux_capture() {
    let Some(cfg) = config_from_env() else {
        eprintln!("THURBOX_SSH_IT != 1 — skipping remote ssh integration test");
        return;
    };

    let host = cfg.host();
    let repo = PathBuf::from(&cfg.repo);

    // 0. Connectivity + fixture sanity: the seeded repo lists `main` over ssh.
    //    This is the `git::*_on` path: `ssh <dest> git -C <repo> branch …`.
    let branches = git::list_branches_on(Some(&host), &repo)
        .expect("list_branches_on over ssh (is the container up? run scripts/test-ssh/up.sh)");
    assert!(
        branches.iter().any(|b| b == "main"),
        "expected seeded `main` branch, got {branches:?}"
    );

    let suffix = unique_suffix();
    let branch = format!("it/{suffix}");
    let marker = format!("THURBOX_REMOTE_MARKER_{suffix}");

    // 1. Create a worktree on a new branch over ssh.
    let wt_path = git::create_worktree_on(Some(&host), &repo, &branch, "main")
        .expect("create_worktree_on over ssh");

    // The returned path lives under the host's configured worktrees dir.
    let wt_str = wt_path.display().to_string();
    assert!(
        wt_str.starts_with(&cfg.worktrees),
        "worktree {wt_str} not under {}",
        cfg.worktrees
    );

    // Guard the rest of the test so the worktree is always cleaned up.
    let result = run_session_and_capture(&cfg, &host, &wt_path, &marker).await;

    // 2. Cleanup: remove the worktree (over ssh) and prune the branch.
    let _ = git::remove_worktree_on(Some(&host), &repo, &wt_path);
    let _ = ssh_exec(
        &cfg,
        &format!(
            "git -C {} branch -D {} || true",
            shell_word(&cfg.repo),
            shell_word(&branch)
        ),
    );

    // The worktree dir should be gone after removal.
    let still_there = Command::new("ssh")
        .args(cfg.ssh_opts())
        .arg(&cfg.destination)
        .arg(format!("test -d {}", shell_word(&wt_str)))
        .status()
        .expect("spawn ssh")
        .success();
    assert!(!still_there, "worktree dir {wt_str} survived removal");

    // Surface any failure from the guarded section.
    result.expect("remote tmux session did not produce the expected marker");
}

/// Spawn a tmux session over ssh in the worktree, running a command that prints
/// `marker`, then poll the live vt100 parser until the marker appears.
async fn run_session_and_capture(
    cfg: &ItConfig,
    host: &HostDef,
    wt_path: &Path,
    marker: &str,
) -> anyhow::Result<()> {
    // Before spawning, confirm the worktree really exists remotely.
    let exists = Command::new("ssh")
        .args(cfg.ssh_opts())
        .arg(&cfg.destination)
        .arg(format!(
            "test -d {}",
            shell_word(&wt_path.display().to_string())
        ))
        .status()
        .expect("spawn ssh")
        .success();
    anyhow::ensure!(exists, "worktree was not created on the remote host");

    // Build the SSH-backed tmux backend for this host and start its session +
    // control mode — the byte-identical control-mode protocol, over ssh.
    let backend: std::sync::Arc<dyn SessionBackend> =
        std::sync::Arc::new(TmuxBackend::from_host(host));
    backend
        .check_available()
        .map_err(|e| anyhow::anyhow!("remote tmux check_available failed: {e:#}"))?;
    backend
        .ensure_ready()
        .map_err(|e| anyhow::anyhow!("remote tmux ensure_ready failed: {e:#}"))?;

    // A trivial stand-in agent. It must emit the marker *continuously*, not
    // once: tmux control mode streams a pane's output via `%output` events only
    // for output produced *after* `connect_pane` subscribes (`refresh-client
    // -A :on`). A single print at startup would land in the pane's screen
    // buffer before we subscribe and never reach the live parser — whereas a
    // real agent repaints continuously. So loop the marker on a short interval.
    // `sh -c` runs as the window's foreground process.
    let provider: std::sync::Arc<dyn thurbox::agent::AgentProvider> =
        std::sync::Arc::new(GenericProvider::new(AgentDef {
            name: "it-shell".into(),
            command: "sh".into(),
            args: vec![
                "-c".into(),
                format!(
                    "i=0; while [ $i -lt 60 ]; do printf '%s\\n' {}; sleep 1; i=$((i+1)); done",
                    shell_word(marker)
                ),
            ],
            resume_args: vec![],
            fork_args: vec![],
            new_session_args: vec![],
        }));

    let config = SessionConfig {
        cwd: Some(wt_path.to_path_buf()),
        agent: "it-shell".into(),
        backend: Some(host.backend_name()),
        ..Default::default()
    };

    let session = Session::spawn(
        format!("ssh-it-{}", std::process::id()),
        24,
        80,
        &config,
        &backend,
        &provider,
    )
    .map_err(|e| anyhow::anyhow!("remote Session::spawn failed: {e:#}"))?;

    // Poll the live parser (fed by the control-mode reader over ssh) for the
    // marker. Generous deadline to absorb image cold-start + ssh latency.
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut found = false;
    while Instant::now() < deadline {
        if let Ok(parser) = session.parser.lock() {
            if parser.screen().contents().contains(marker) {
                found = true;
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    // Kill the remote pane regardless of outcome.
    session.kill();

    anyhow::ensure!(
        found,
        "marker `{marker}` never appeared in remote pane output"
    );
    Ok(())
}

/// POSIX single-quote a token for embedding in a raw `ssh … <cmd>` string used
/// by the test harness (assertions/cleanup only).
fn shell_word(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
