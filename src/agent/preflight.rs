//! Best-effort pre-flight: is an agent's `command` runnable before we spawn a
//! tmux window for it?
//!
//! A missing binary (a typo in `agents.toml`, or a seeded-but-never-installed
//! built-in like `agy`/`copilot`/`vibe`) makes the window's shell exit instantly
//! with `command not found` and the pane dies — with no upfront error in the
//! new-session flow. This module resolves the `command` the way a shell would so
//! the spawn paths can catch that up front.
//!
//! **Advisory, never authoritative.** The check answers only "is this `command`
//! runnable?", agent-neutrally. Any uncertainty — an unreadable `$PATH`, a
//! `command` that's actually a shell function/alias the walk can't see —
//! resolves to "assume present" so a valid spawn is never blocked. Callers that
//! can prompt (the TUI) treat a negative as advisory (confirm, not block); the
//! scriptable headless path hard-errors.

/// Best-effort: is `command` on `PATH` (or a runnable path)?
///
/// Returns `false` **only** when we can confidently say the binary is not
/// resolvable; every uncertain case returns `true`. Mirrors shell resolution:
/// a `command` containing a path separator is checked directly, a bare name is
/// looked up across `$PATH` (and, on Windows, each `PATHEXT` extension).
pub fn command_on_path(command: &str) -> bool {
    let path = std::env::var("PATH").ok();
    #[cfg(windows)]
    let pathext =
        Some(std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string()));
    #[cfg(not(windows))]
    let pathext: Option<String> = None;
    resolve(command, path.as_deref(), pathext.as_deref(), &is_executable)
}

/// The real filesystem executability predicate. On Unix a candidate must be a
/// file with an execute bit set; elsewhere (Windows) the extension has already
/// been appended by [`resolve`], so a plain file-exists check is enough.
#[cfg(unix)]
fn is_executable(candidate: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(candidate)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(candidate: &str) -> bool {
    std::path::Path::new(candidate).is_file()
}

/// Pure PATH resolution, factored out so it's testable with an injected `$PATH`,
/// `PATHEXT`, and a fake `exists` predicate — independent of the host OS.
///
/// `pathext` being `Some` selects Windows semantics: `;`-separated `PATH`,
/// backslash as a path separator, and each extension appended when a candidate
/// has no direct hit. `None` selects POSIX semantics (`:`-separated, `/` only).
fn resolve(
    command: &str,
    path: Option<&str>,
    pathext: Option<&str>,
    exists: &dyn Fn(&str) -> bool,
) -> bool {
    // An empty command is nonsensical to probe; don't block on it.
    if command.is_empty() {
        return true;
    }

    let windows = pathext.is_some();
    let list_sep = if windows { ';' } else { ':' };
    let dir_sep = if windows { '\\' } else { '/' };
    let exts: Vec<&str> = pathext
        .map(|pe| pe.split(';').filter(|e| !e.is_empty()).collect())
        .unwrap_or_default();
    let is_sep = |c: char| c == '/' || (windows && c == '\\');

    // Does `base` (or `base` + a PATHEXT extension, on Windows) resolve?
    let hit = |base: &str| -> bool {
        if exists(base) {
            return true;
        }
        windows && exts.iter().any(|ext| exists(&format!("{base}{ext}")))
    };

    // A command with a path separator is resolved directly, never via PATH.
    if command.chars().any(is_sep) {
        return hit(command);
    }

    // A bare name is looked up across PATH. If PATH is unreadable/empty we can't
    // judge — assume present.
    let Some(path) = path.filter(|p| !p.is_empty()) else {
        return true;
    };
    path.split(list_sep)
        .filter(|dir| !dir.is_empty())
        .any(|dir| hit(&format!("{dir}{dir_sep}{command}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Build an `exists` predicate from a fixed set of resolvable paths.
    fn present(paths: &[&str]) -> impl Fn(&str) -> bool {
        let set: HashSet<String> = paths.iter().map(|s| s.to_string()).collect();
        move |p: &str| set.contains(p)
    }

    #[test]
    fn bare_name_found_in_path() {
        let exists = present(&["/home/me/.local/bin/claude"]);
        assert!(resolve(
            "claude",
            Some("/usr/bin:/home/me/.local/bin"),
            None,
            &exists
        ));
    }

    #[test]
    fn bare_name_not_found_is_confidently_missing() {
        let exists = present(&["/usr/bin/ls"]);
        assert!(!resolve("agy", Some("/usr/bin:/bin"), None, &exists));
    }

    #[test]
    fn path_separator_command_checked_directly() {
        let exists = present(&["/opt/agent/claude"]);
        assert!(resolve("/opt/agent/claude", None, None, &exists));
        assert!(!resolve("/opt/agent/missing", None, None, &exists));
        // A directory-separator command never falls back to a PATH walk.
        assert!(!resolve(
            "./claude",
            Some("/dir-with-claude"),
            None,
            &present(&["/dir-with-claude/claude"])
        ));
    }

    #[test]
    fn unreadable_or_empty_path_assumes_present() {
        let exists = present(&[]);
        assert!(resolve("claude", None, None, &exists));
        assert!(resolve("claude", Some(""), None, &exists));
    }

    #[test]
    fn empty_command_assumes_present() {
        assert!(resolve("", Some("/usr/bin"), None, &present(&[])));
    }

    #[test]
    fn windows_appends_pathext() {
        let exists = present(&["C:\\bin\\agy.EXE"]);
        // Bare name resolves only after a PATHEXT extension is appended.
        assert!(resolve(
            "agy",
            Some("C:\\bin"),
            Some(".COM;.EXE;.CMD"),
            &exists
        ));
        assert!(!resolve("agy", Some("C:\\bin"), Some(".COM;.CMD"), &exists));
    }

    #[test]
    fn windows_direct_extension_and_backslash_separator() {
        // An explicit extension resolves directly.
        let exists = present(&["C:\\tools\\claude.cmd"]);
        assert!(resolve(
            "C:\\tools\\claude.cmd",
            None,
            Some(".EXE;.CMD"),
            &exists
        ));
        // A backslash marks a path command under Windows semantics (no PATH walk);
        // the extension is appended. (`resolve` matches the extension verbatim —
        // the real predicate defers case-insensitivity to the filesystem — so the
        // fixture keeps PATHEXT and the file casing consistent.)
        let exists = present(&["C:\\tools\\claude.CMD"]);
        assert!(resolve(
            "C:\\tools\\claude",
            Some("C:\\other"),
            Some(".CMD"),
            &exists
        ));
    }
}
