//! Small shell/SSH command helpers shared across modules.
//!
//! Centralizes two things that would otherwise be duplicated wherever thurbox
//! shells out over SSH: POSIX single-quote escaping for tokens that a remote
//! login shell will re-split, and construction of the `ssh <opts> <dest>`
//! command prefix.

use std::process::Command;

/// Characters that never need quoting in a POSIX shell word.
fn is_safe_shell_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | ':' | '=' | ',')
}

/// POSIX single-quote escaping for one shell word.
///
/// Simple tokens (paths, branch names, flags) pass through unquoted; anything
/// with whitespace or shell metacharacters is wrapped in single quotes (with
/// embedded quotes escaped). An empty string becomes `''`.
///
/// Note: this does **not** strip newlines. Callers feeding a line-delimited
/// protocol (e.g. tmux control mode) must handle newlines themselves before
/// quoting — see [`crate::agent::control_mode::shell_escape`].
pub fn posix_quote(s: &str) -> String {
    if !s.is_empty() && s.chars().all(is_safe_shell_char) {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Build an `ssh <opts> <destination>` [`Command`], ready for the caller to
/// append the remote command and its arguments.
pub fn ssh_command(destination: &str, ssh_opts: &[String]) -> Command {
    let mut cmd = Command::new("ssh");
    cmd.args(ssh_opts);
    cmd.arg(destination);
    cmd
}

/// Build a `wsl.exe -d <distro>` [`Command`], ready for the caller to append
/// the in-distro command and its arguments.
///
/// This is the WSL analogue of [`ssh_command`]: `wsl.exe` joins the trailing
/// arguments with spaces and runs them through the distro's default login
/// shell (it only bypasses the shell with `-e`/`--exec`, which we don't pass),
/// exactly like `ssh <dest> <command>`. So callers append the *same*
/// POSIX-quoted tokens they would for SSH — the in-distro shell re-splits them
/// identically. No `--` separator is used (none of thurbox's commands start
/// with a `-`, matching the SSH path which also omits it), so distros are
/// reached uniformly on every supported `wsl.exe`.
///
/// `--cd /` pins the launched process's working directory to a path that exists
/// on *every* distro. Without it, `wsl.exe` inherits the **caller's** current
/// directory and tries to `chdir` to the same path inside the target distro —
/// which fails when thurbox itself runs inside another WSL distro (the caller's
/// cwd, e.g. `/home/me/repo`, doesn't exist on the target). That failure prints
/// a `WSL Relay ERROR: CreateProcessCommon chdir(...) failed` on stderr and
/// corrupts the tmux control-mode handshake, so the agent window never launches.
/// `/` is a neutral landing dir: every caller overrides the real working dir
/// anyway (`git -C <path>`, tmux `new-window -c <path>`).
pub fn wsl_command(distro: &str) -> Command {
    let mut cmd = Command::new("wsl.exe");
    cmd.arg("-d").arg(distro);
    cmd.arg("--cd").arg("/");
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn posix_quote_passes_simple_tokens() {
        assert_eq!(posix_quote("feat-x"), "feat-x");
        assert_eq!(posix_quote("/home/me/repo"), "/home/me/repo");
        assert_eq!(posix_quote("-L"), "-L");
    }

    #[test]
    fn posix_quote_wraps_specials_and_empty() {
        assert_eq!(posix_quote("a b"), "'a b'");
        assert_eq!(posix_quote("it's"), "'it'\\''s'");
        assert_eq!(posix_quote(""), "''");
    }

    #[test]
    fn ssh_command_sets_program_opts_and_destination() {
        let cmd = ssh_command("me@box", &["-p".into(), "2222".into()]);
        assert_eq!(cmd.get_program().to_string_lossy(), "ssh");
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args, ["-p", "2222", "me@box"]);
    }

    #[test]
    fn wsl_command_sets_program_distro_and_neutral_cwd() {
        let cmd = wsl_command("Ubuntu");
        assert_eq!(cmd.get_program().to_string_lossy(), "wsl.exe");
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        // `--cd /` pins a valid landing dir so wsl.exe doesn't inherit (and fail
        // to chdir to) the caller's cwd when thurbox runs inside another distro.
        assert_eq!(args, ["-d", "Ubuntu", "--cd", "/"]);
    }
}
