//! Per-session multi-repo **symlink workspaces**.
//!
//! When a session spans more than one directory (multiple repos, or a repo plus
//! an extra access dir), the agent process can only be launched in a single
//! `cwd`. Rather than teach every agent CLI a different `--add-dir`-style flag
//! (many have none), thurbox builds one workspace directory full of symlinks —
//! one per member dir — and launches the agent there. The agent then sees every
//! repo as a subdirectory, with no per-agent configuration.
//!
//! ```text
//! ~/.local/share/thurbox/workspaces/<agent_session_id>/
//!     webapp  -> …/worktrees/<hash>/feat-x   (symlink)
//!     infra   -> /home/me/repos/infra        (symlink)
//! ```
//!
//! The directory only ever contains symlinks, so tearing it down (or rebuilding
//! it) never touches the underlying repositories. The path is derived from the
//! session's stable `agent_session_id`, so it is rebuilt idempotently on every
//! launch and needs no separate persistence.

use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};

use crate::paths;

/// Resolve the workspace directory for a session id, ensuring it stays a single
/// segment under the workspaces root (defensive — the id is a UUID in practice).
fn workspace_dir(id: &str) -> io::Result<PathBuf> {
    let base = paths::workspaces_directory().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "could not resolve workspaces directory",
        )
    })?;
    let segment = sanitize_segment(id);
    if segment.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "empty workspace id",
        ));
    }
    Ok(base.join(segment))
}

/// The directory the agent process should launch in.
///
/// For a single-member session that's the member itself (`primary_cwd`). For
/// a multi-member session it is the per-session **symlink workspace** (built
/// idempotently from the members via [`ensure_workspace`]) so the agent sees
/// every repo as a subdirectory — agent-neutral, needing no per-CLI flag.
/// Member labels fall back to the directory's file name (then `"repo"`).
/// Without an `agent_session_id` there is no stable workspace name, and on
/// any build failure, it falls back to `primary_cwd`.
pub fn resolve_process_cwd(
    agent_session_id: Option<&str>,
    primary_cwd: Option<PathBuf>,
    members: &[(Option<String>, PathBuf)],
) -> Option<PathBuf> {
    if members.len() < 2 {
        return primary_cwd;
    }
    let Some(id) = agent_session_id else {
        return primary_cwd;
    };

    let pairs: Vec<(String, PathBuf)> = members
        .iter()
        .map(|(name, dir)| {
            let label = name
                .clone()
                .or_else(|| dir.file_name().and_then(|s| s.to_str()).map(String::from))
                .unwrap_or_else(|| "repo".to_string());
            (label, dir.clone())
        })
        .collect();

    match ensure_workspace(id, &pairs) {
        Ok(ws) => Some(ws),
        Err(e) => {
            tracing::error!("Failed to build multi-repo workspace: {e}");
            primary_cwd
        }
    }
}

/// (Re)build the symlink workspace for `id` from `members` and return its path.
///
/// Idempotent: any existing workspace dir is removed first (it holds only
/// symlinks, so targets are untouched) and recreated from scratch, so adding or
/// removing a member between launches is reflected. Each member is symlinked
/// under a sanitized, de-duplicated name (collisions get a `-2`, `-3`, … suffix).
pub fn ensure_workspace(id: &str, members: &[(String, PathBuf)]) -> io::Result<PathBuf> {
    let dir = workspace_dir(id)?;
    remove_dir_under_root(&dir)?;
    std::fs::create_dir_all(&dir)?;

    let mut used: HashSet<String> = HashSet::new();
    for (name, target) in members {
        let link_name = unique_link_name(name, &mut used);
        let link_path = dir.join(&link_name);
        symlink(target, &link_path)?;
    }

    Ok(dir)
}

/// Remove the workspace directory for `id`, if present. Only the symlinks are
/// removed; the directories they point at are untouched. A missing workspace is
/// not an error.
pub fn remove_workspace(id: &str) -> io::Result<()> {
    let dir = workspace_dir(id)?;
    remove_dir_under_root(&dir)
}

/// Remove `dir` (recursively) only when it really sits under the workspaces
/// root, so a bad id can never delete something outside it. `remove_dir_all`
/// unlinks symlink entries without following them, so member repos are safe.
fn remove_dir_under_root(dir: &Path) -> io::Result<()> {
    let Some(base) = paths::workspaces_directory() else {
        return Ok(());
    };
    if !dir.starts_with(&base) || dir == base {
        return Ok(());
    }
    match std::fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Create a symlink at `link` pointing to `target` (Unix; thurbox targets
/// Linux + macOS only).
fn symlink(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

/// Reduce a repo display name to a safe single path segment for a link name.
fn sanitize_segment(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' => '-',
            c if c.is_whitespace() => '-',
            c => c,
        })
        .collect();
    cleaned.trim_matches(['.', '-']).to_string()
}

/// Sanitize `name` and make it unique within `used` by appending `-2`, `-3`, …
fn unique_link_name(name: &str, used: &mut HashSet<String>) -> String {
    let base = {
        let s = sanitize_segment(name);
        if s.is_empty() {
            "repo".to_string()
        } else {
            s
        }
    };
    if used.insert(base.clone()) {
        return base;
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}-{n}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::TestPathGuard;

    fn temp_base() -> PathBuf {
        // Unique-ish per test via the thread name; avoids Date/rand (forbidden).
        let mut p = std::env::temp_dir();
        let t = std::thread::current();
        let name = t.name().unwrap_or("ws").replace("::", "-");
        p.push(format!("thurbox-ws-test-{name}"));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn ensure_creates_one_symlink_per_member() {
        let base = temp_base();
        let _g = TestPathGuard::new(&base);
        let repo_a = base.join("src-a");
        let repo_b = base.join("src-b");
        std::fs::create_dir_all(&repo_a).unwrap();
        std::fs::create_dir_all(&repo_b).unwrap();

        let ws = ensure_workspace(
            "sess-1",
            &[
                ("webapp".to_string(), repo_a.clone()),
                ("infra".to_string(), repo_b.clone()),
            ],
        )
        .unwrap();

        assert!(ws.ends_with("workspaces/sess-1"));
        assert_eq!(std::fs::read_link(ws.join("webapp")).unwrap(), repo_a);
        assert_eq!(std::fs::read_link(ws.join("infra")).unwrap(), repo_b);
    }

    #[test]
    fn ensure_is_idempotent_and_reflects_new_members() {
        let base = temp_base();
        let _g = TestPathGuard::new(&base);
        let a = base.join("a");
        let b = base.join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();

        ensure_workspace("s", &[("a".into(), a.clone())]).unwrap();
        let ws =
            ensure_workspace("s", &[("a".into(), a.clone()), ("b".into(), b.clone())]).unwrap();

        assert!(ws.join("a").exists());
        assert!(ws.join("b").exists());
    }

    #[test]
    fn colliding_names_are_disambiguated() {
        let base = temp_base();
        let _g = TestPathGuard::new(&base);
        let a = base.join("one");
        let b = base.join("two");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();

        let ws = ensure_workspace(
            "s",
            &[("repo".into(), a.clone()), ("repo".into(), b.clone())],
        )
        .unwrap();

        assert_eq!(std::fs::read_link(ws.join("repo")).unwrap(), a);
        assert_eq!(std::fs::read_link(ws.join("repo-2")).unwrap(), b);
    }

    #[test]
    fn remove_deletes_links_not_targets() {
        let base = temp_base();
        let _g = TestPathGuard::new(&base);
        let repo = base.join("keepme");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::write(repo.join("file.txt"), b"data").unwrap();

        let ws = ensure_workspace("s", &[("repo".into(), repo.clone())]).unwrap();
        assert!(ws.exists());

        remove_workspace("s").unwrap();
        assert!(!ws.exists());
        // Target survived.
        assert!(repo.join("file.txt").exists());
    }

    #[test]
    fn remove_missing_workspace_is_ok() {
        let base = temp_base();
        let _g = TestPathGuard::new(&base);
        assert!(remove_workspace("never-made").is_ok());
    }

    #[test]
    fn sanitize_strips_separators_and_dots() {
        assert_eq!(sanitize_segment("a/b\\c:d"), "a-b-c-d");
        assert_eq!(sanitize_segment("  .git  "), "git");
        assert_eq!(sanitize_segment("my repo"), "my-repo");
    }

    #[test]
    fn resolve_single_member_is_primary() {
        let cwd = PathBuf::from("/src/only");
        let members = vec![(None, cwd.clone())];
        assert_eq!(
            resolve_process_cwd(Some("id-1"), Some(cwd.clone()), &members),
            Some(cwd)
        );
    }

    #[test]
    fn resolve_multi_member_without_id_falls_back_to_primary() {
        let primary = PathBuf::from("/src/a");
        let members = vec![(None, primary.clone()), (None, PathBuf::from("/src/b"))];
        assert_eq!(
            resolve_process_cwd(None, Some(primary.clone()), &members),
            Some(primary)
        );
    }

    #[test]
    fn resolve_multi_member_builds_workspace_with_file_name_labels() {
        let base = temp_base();
        let _g = TestPathGuard::new(&base);
        let primary = base.join("repo-a");
        let other = base.join("repo-b");
        std::fs::create_dir_all(&primary).unwrap();
        std::fs::create_dir_all(&other).unwrap();

        let members = vec![
            (Some("named".to_string()), primary.clone()),
            (None, other.clone()),
        ];
        let out = resolve_process_cwd(Some("sess-y"), Some(primary.clone()), &members).unwrap();

        let ws_root = crate::paths::workspaces_directory().unwrap();
        assert!(out.starts_with(&ws_root), "{out:?} not under {ws_root:?}");
        assert_eq!(std::fs::read_link(out.join("named")).unwrap(), primary);
        // Unnamed member labeled by its directory file name.
        assert_eq!(std::fs::read_link(out.join("repo-b")).unwrap(), other);
    }
}
