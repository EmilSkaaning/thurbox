//! Creating a session end to end, against a real git repository.
//!
//! This is the one test that runs the whole pipeline for real: a repo on disk,
//! a worktree, a tmux window, a process. It is skipped when tmux is absent
//! rather than failing, because a missing multiplexer is an environment fact,
//! not a regression — but it *runs* wherever tmux exists, including CI.
//!
//! Everything it touches is scoped to a throwaway socket and a temporary
//! directory, so it can never disturb a real session.

use std::process::Command;

mod common;

use common::git;

/// A throwaway tmux socket, so this never touches the real one.
const SOCKET: &str = "thurbox-create-e2e";

fn have_tmux() -> bool {
    Command::new("tmux")
        .arg("-V")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// A repository with one commit, which is the minimum a worktree needs.
fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    git(dir.path(), &["init", "-q", "-b", "main"]);
    git(dir.path(), &["config", "user.email", "t@example.com"]);
    git(dir.path(), &["config", "user.name", "thurbox-test"]);
    // Signing would make this depend on a key in the user's agent, and the repo
    // is throwaway.
    git(dir.path(), &["config", "commit.gpgsign", "false"]);
    std::fs::write(dir.path().join("README.md"), "# probe\n").expect("write");
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "init"]);
    dir
}

fn cleanup() {
    let _ = Command::new("tmux")
        .args(["-L", SOCKET, "kill-server"])
        .output();
}

#[test]
fn creating_a_session_produces_a_worktree_a_row_and_a_window() {
    if !have_tmux() {
        eprintln!("skipping: tmux is not installed");
        return;
    }

    let repo = repo();
    let db = thurbox::storage::Database::open_in_memory().expect("db");

    // Isolate config and data, so this uses neither the real agents.toml nor
    // the real database. nextest runs each test in its own process, so a
    // process-wide path override is safe here.
    let home = tempfile::tempdir().expect("tempdir");
    thurbox::paths::set_test_dir(home.path());

    // A shell rather than a real agent: the pipeline is what is under test, and
    // launching a coding agent would want credentials and a network.
    let config = thurbox::paths::config_file()
        .expect("config path")
        .parent()
        .expect("config dir")
        .to_path_buf();
    std::fs::create_dir_all(&config).expect("mkdir");
    std::fs::write(
        config.join("agents.toml"),
        "default = \"shell\"\n\n[[agents]]\nname = \"shell\"\ncommand = \"sh\"\nargs = []\n",
    )
    .expect("write agents.toml");

    let result = thurbox::session_ops::spawn::spawn_session_headless(
        &db,
        thurbox::session_ops::spawn::SpawnRequest {
            name: "e2e-probe".into(),
            repo_path: repo.path().to_path_buf(),
            worktree_branch: Some("feat/e2e".into()),
            base_branch: Some("main".into()),
            agent: Some("shell".into()),
            agent_session_id: None,
            host: None,
            parent_session_id: None,
            task_id: None,
            extra_repos: Vec::new(),
            fork_session_id: None,
            inherit_worktrees: Vec::new(),
        },
    );

    let spawned = match result {
        Ok(spawned) => spawned,
        Err(e) => {
            cleanup();
            // A server that will not start is an environment fact, like a
            // missing binary. Anything else is this pipeline breaking, and this
            // is the only test that runs the whole of it — so the skip has to
            // name the condition rather than the tool. Matching the substring
            // "tmux" over the whole error exited GREEN on every backend failure
            // that merely mentions it ("can't find pane", "failed to create
            // window", any `reportable_stderr` line), which is the one outcome
            // an end-to-end test must never have.
            const NO_SERVER: [&str; 3] = [
                "no server running",
                "failed to connect to server",
                "error connecting to",
            ];
            if NO_SERVER.iter().any(|marker| e.contains(marker)) {
                eprintln!("skipping: no multiplexer server to spawn into: {e}");
                return;
            }
            panic!("creation failed: {e}");
        }
    };

    // The row exists, and carries what a plugin needs to draw it.
    let row = db
        .get_session_by_id(spawned.session_id)
        .expect("query")
        .expect("the session should be persisted");
    assert_eq!(row.name, "e2e-probe");
    assert_eq!(row.agent, "shell");

    // The worktree exists on disk, on the branch that was asked for.
    let worktree = row
        .worktrees
        .first()
        .expect("a branch was requested, so there must be a worktree");
    assert_eq!(worktree.branch, "feat/e2e");
    assert!(
        worktree.worktree_path.is_dir(),
        "{} is not a directory",
        worktree.worktree_path.display()
    );
    assert!(
        worktree.worktree_path.join("README.md").is_file(),
        "the worktree should carry the repo's content"
    );

    // And the snapshot the kernel publishes sees it.
    let store = thurbox::kernel::snapshot::SnapshotStore::with_database(db);
    let published = store
        .current()
        .sessions
        .iter()
        .find(|s| s.name == "e2e-probe")
        .expect("the new session should reach the snapshot");
    assert_eq!(published.branch.as_deref(), Some("feat/e2e"));

    cleanup();
}
