//! Best-effort "is this command runnable?" detection.
//!
//! A single, agent-neutral primitive that asks the OS whether an
//! [`AgentDef`](crate::session::AgentDef)'s opaque `command` string resolves to
//! an executable. It walks `$PATH` exactly as a shell would (plus `PATHEXT` on
//! Windows), so it never bakes any agent's name or flags into the binary.
//!
//! It powers two point-in-time observations: the seed-time default/annotation
//! choice ([`crate::agent::agent_config`]) and the pick-time picker dimming
//! (`finish_prepare_spawn`). Both re-probe each time — a PATH walk is
//! microseconds — so a freshly-installed agent is picked up on the next run
//! without any caching.
//!
//! Detection is deliberately advisory: a shell function / alias / rc-resolved
//! command can false-negative here, so callers use it to *inform* display and
//! seeding, never to hard-block (the spawn-time guard that can block is a
//! separate concern).

/// Whether `command` resolves to a runnable executable using the process's
/// current environment (`PATH`, and `PATHEXT` on Windows).
///
/// Returns `true` when the command can't be conclusively ruled out — an
/// unreadable/absent `PATH` degrades to "assume present" so detection never
/// makes the caller *more* pessimistic than reality.
pub fn command_on_path(command: &str) -> bool {
    let path = std::env::var_os("PATH");
    // On non-Windows, `pathext` is unused; on Windows it drives the extension
    // probe. Reading it unconditionally keeps the pure inner fn signature flat.
    let pathext = std::env::var_os("PATHEXT");
    command_on_path_in(command, path.as_deref(), pathext.as_deref())
}

/// Pure core of [`command_on_path`], parameterized on the `PATH` / `PATHEXT`
/// values so it can be tested hermetically without mutating process env.
fn command_on_path_in(
    command: &str,
    path_var: Option<&std::ffi::OsStr>,
    pathext: Option<&std::ffi::OsStr>,
) -> bool {
    if command.is_empty() {
        return false;
    }

    // A command carrying a path separator is resolved directly, not via PATH —
    // mirroring how the shell treats `./x` or `/usr/bin/x`.
    if has_path_separator(command) {
        return is_executable_file(std::path::Path::new(command), pathext);
    }

    // No PATH to walk: we can't prove absence, so assume present (advisory).
    let Some(path_var) = path_var else {
        return true;
    };

    std::env::split_paths(path_var).any(|dir| {
        // Skip the empty entries some PATHs carry (a leading/trailing `:`).
        !dir.as_os_str().is_empty() && is_executable_file(&dir.join(command), pathext)
    })
}

/// Whether `path` (a fully-formed candidate) is an executable file. On Windows,
/// also try each `PATHEXT` extension when the bare path doesn't already resolve.
fn is_executable_file(path: &std::path::Path, pathext: Option<&std::ffi::OsStr>) -> bool {
    if is_runnable(path) {
        return true;
    }
    if cfg!(windows) {
        for ext in windows_pathext(pathext) {
            // Extensions in PATHEXT include the leading dot (".EXE").
            let mut candidate = path.as_os_str().to_owned();
            candidate.push(&ext);
            if is_runnable(std::path::Path::new(&candidate)) {
                return true;
            }
        }
    }
    false
}

/// Whether `path` exists as a regular file that a shell could launch. A
/// metadata error (missing / unreadable) or a non-file is *not* runnable; on
/// Unix an execute bit is also required, while Windows has no such bit so a
/// regular file suffices (PATHEXT is applied by the caller).
fn is_runnable(path: &std::path::Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Any execute bit (user/group/other) makes it runnable enough for a
        // shell to launch it.
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        // No executable bit on Windows: existence as a regular file (already
        // checked, with PATHEXT applied upstream) is the runnable signal.
        true
    }
}

/// The Windows executable extensions to try, from `PATHEXT`, falling back to a
/// sensible default set when the var is missing/empty.
fn windows_pathext(pathext: Option<&std::ffi::OsStr>) -> Vec<std::ffi::OsString> {
    let raw = pathext
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".to_string());
    raw.split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(std::ffi::OsString::from)
        .collect()
}

/// Whether the command string carries a path separator (so it should be
/// resolved as a path, not looked up in `PATH`). Backslash counts on Windows.
fn has_path_separator(command: &str) -> bool {
    command.contains('/') || (cfg!(windows) && command.contains('\\'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    /// Create an executable file `name` inside `dir`.
    fn write_exe(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, b"#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        p
    }

    #[test]
    fn found_when_present_on_path() {
        let dir = tempfile::TempDir::new().unwrap();
        write_exe(dir.path(), "my-agent");
        let path = dir.path().as_os_str();
        assert!(command_on_path_in("my-agent", Some(path), None));
    }

    #[test]
    fn missing_when_absent_from_path() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().as_os_str();
        assert!(!command_on_path_in("nope-not-here", Some(path), None));
    }

    #[test]
    fn empty_command_is_never_found() {
        let dir = tempfile::TempDir::new().unwrap();
        assert!(!command_on_path_in("", Some(dir.path().as_os_str()), None));
    }

    #[test]
    fn absent_path_var_assumes_present() {
        // Advisory posture: with no PATH to walk we can't prove absence.
        assert!(command_on_path_in("anything", None, None));
    }

    #[test]
    fn empty_path_entries_are_skipped() {
        let dir = tempfile::TempDir::new().unwrap();
        write_exe(dir.path(), "agent");
        // A PATH with empty segments (leading/trailing separators) must still
        // find the real dir and not treat "" as the cwd match.
        let sep = if cfg!(windows) { ";" } else { ":" };
        let joined = format!("{sep}{}{sep}", dir.path().display());
        assert!(command_on_path_in("agent", Some(OsStr::new(&joined)), None));
        assert!(!command_on_path_in(
            "ghost",
            Some(OsStr::new(&joined)),
            None
        ));
    }

    #[cfg(unix)]
    #[test]
    fn path_containing_command_checks_directly() {
        let dir = tempfile::TempDir::new().unwrap();
        let exe = write_exe(dir.path(), "direct");
        let as_path = exe.to_str().unwrap();
        // An absolute/relative path is resolved directly, ignoring PATH.
        assert!(command_on_path_in(as_path, None, None));
        let missing = dir.path().join("gone");
        assert!(!command_on_path_in(missing.to_str().unwrap(), None, None));
    }

    #[cfg(unix)]
    #[test]
    fn non_executable_file_is_not_runnable() {
        let dir = tempfile::TempDir::new().unwrap();
        let p = dir.path().join("data");
        std::fs::write(&p, b"x").unwrap(); // no +x bit
        assert!(!command_on_path_in(
            "data",
            Some(dir.path().as_os_str()),
            None
        ));
    }

    // `windows_pathext` is pure and compiled on every platform (the caller
    // gates it behind `cfg!(windows)`, not the fn), so its parsing is testable
    // regardless of host OS.

    #[test]
    fn windows_pathext_falls_back_to_defaults() {
        assert_eq!(
            windows_pathext(None),
            [".COM", ".EXE", ".BAT", ".CMD"].map(std::ffi::OsString::from)
        );
        // Blank/whitespace-only PATHEXT also uses the defaults.
        assert_eq!(
            windows_pathext(Some(OsStr::new("   "))),
            [".COM", ".EXE", ".BAT", ".CMD"].map(std::ffi::OsString::from)
        );
    }

    #[test]
    fn windows_pathext_parses_trims_and_skips_empties() {
        assert_eq!(
            windows_pathext(Some(OsStr::new(".EXE; .CMD ;;.PS1"))),
            [".EXE", ".CMD", ".PS1"].map(std::ffi::OsString::from)
        );
    }
}
