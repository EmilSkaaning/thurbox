//! Environment a plugin may contribute to spawned agent sessions.
//!
//! A spawn contribution lets a plugin add environment variables to the agent
//! processes thurbox starts. Append-only, veto-free contributions bound *who
//! wins* a conflict — they do not bound *what can be injected*, and that
//! distinction is the whole reason this module exists.
//!
//! Several environment variables are, in effect, "run this code in every agent
//! session": `LD_PRELOAD` and `DYLD_INSERT_LIBRARIES` load a shared object into
//! every child, `BASH_ENV` sources a script into every non-interactive shell,
//! `GIT_SSH_COMMAND` and `GIT_EXTERNAL_DIFF` replace programs git invokes, and
//! `NODE_OPTIONS` can `--require` a module into every Node process. A plugin
//! that could set any of them would have arbitrary code execution inside every
//! session thurbox spawns, which is a far larger grant than "may add some
//! environment".
//!
//! So the policy is enforced here, as pure data, and the rejections are kept
//! rather than dropped — a contribution silently discarded is a plugin that
//! looks installed and does nothing.

use std::collections::BTreeMap;
use std::path::Path;

/// Environment variables a contribution may never set.
///
/// Each entry is a code-execution vector, not merely a sensitive value. The
/// list is deliberately literal rather than pattern-based: a pattern that
/// happened to be too narrow would fail open, and every entry here is worth
/// naming explicitly so a reader can see *why* it is refused.
pub const DENIED_ENV_KEYS: &[&str] = &[
    // Loads an arbitrary shared object into every child process.
    "LD_PRELOAD",
    "LD_AUDIT",
    "LD_LIBRARY_PATH",
    // The macOS equivalents.
    "DYLD_INSERT_LIBRARIES",
    "DYLD_LIBRARY_PATH",
    // Sourced by every non-interactive bash; likewise for zsh/ENV.
    "BASH_ENV",
    "ENV",
    // Replace programs git itself invokes.
    "GIT_SSH",
    "GIT_SSH_COMMAND",
    "GIT_EXTERNAL_DIFF",
    "GIT_PAGER",
    "GIT_EDITOR",
    // `--require` runs a module in every Node process.
    "NODE_OPTIONS",
    // Python's equivalent of BASH_ENV.
    "PYTHONSTARTUP",
    "PYTHONPATH",
    // Perl's.
    "PERL5OPT",
    "PERL5LIB",
    // A general "run this instead" hook for many tools.
    "EDITOR",
    "VISUAL",
    "PAGER",
    "SHELL",
];

/// Why one contributed variable was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    /// The key is on [`DENIED_ENV_KEYS`].
    DeniedKey {
        /// The plugin that tried.
        plugin: String,
        /// The key it tried to set.
        key: String,
    },
    /// `PATH` was contributed with an entry outside the plugin's own data
    /// directory.
    PathEscape {
        /// The plugin that tried.
        plugin: String,
        /// The offending entry.
        entry: String,
    },
    /// Another plugin already set this key, and contributions are append-only.
    AlreadySet {
        /// The plugin that lost.
        plugin: String,
        /// The key.
        key: String,
        /// The plugin that set it first.
        winner: String,
    },
}

impl std::fmt::Display for Rejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Rejection::DeniedKey { plugin, key } => write!(
                f,
                "plugin `{plugin}` may not set `{key}`: it would run code in every agent session"
            ),
            Rejection::PathEscape { plugin, entry } => write!(
                f,
                "plugin `{plugin}` may only prepend PATH entries under its own data directory, not `{entry}`"
            ),
            Rejection::AlreadySet {
                plugin,
                key,
                winner,
            } => write!(
                f,
                "plugin `{plugin}` cannot set `{key}`: already set by `{winner}` (contributions are append-only)"
            ),
        }
    }
}

/// One plugin's proposed additions to a spawn's environment.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpawnContribution {
    /// The contributing plugin.
    pub plugin: String,
    /// Variables it wants set.
    pub env: BTreeMap<String, String>,
    /// Directories it wants prepended to `PATH`.
    pub path_prepends: Vec<String>,
}

/// The environment additions that survived policy, plus what did not.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedContributions {
    /// Variables to add to the spawn, keyed by name.
    pub env: BTreeMap<String, String>,
    /// `PATH` entries to prepend, in contribution order.
    pub path_prepends: Vec<String>,
    /// Everything refused, with its reason. Surfaced rather than dropped: a
    /// contribution that vanishes silently is a plugin that looks installed
    /// and does nothing.
    pub rejections: Vec<Rejection>,
}

/// Apply the contribution policy.
///
/// `data_dir_for` maps a plugin name to the directory it owns; a `PATH`
/// prepend is allowed only under that directory, so a plugin can put its own
/// helper binaries in front without being able to shadow `git` or `ssh` with
/// something it does not control.
///
/// Contributions are applied in the order given and are **append-only**: the
/// first plugin to set a key keeps it. That bounds who wins a conflict; the
/// denylist above is what bounds what may be injected at all.
pub fn resolve<'a>(
    contributions: impl IntoIterator<Item = &'a SpawnContribution>,
    data_dir_for: impl Fn(&str) -> Option<std::path::PathBuf>,
) -> ResolvedContributions {
    let mut out = ResolvedContributions::default();
    // Who set each key, so a later contributor can be told who beat it.
    let mut owner: BTreeMap<String, String> = BTreeMap::new();

    for contribution in contributions {
        for (key, value) in &contribution.env {
            if is_denied(key) {
                out.rejections.push(Rejection::DeniedKey {
                    plugin: contribution.plugin.clone(),
                    key: key.clone(),
                });
                continue;
            }
            // `PATH` is never settable outright — only prependable, and only
            // under the plugin's own directory. Setting it wholesale would
            // replace the lookup path for every program the agent runs.
            if key.eq_ignore_ascii_case("PATH") {
                out.rejections.push(Rejection::DeniedKey {
                    plugin: contribution.plugin.clone(),
                    key: key.clone(),
                });
                continue;
            }
            if let Some(winner) = owner.get(key) {
                out.rejections.push(Rejection::AlreadySet {
                    plugin: contribution.plugin.clone(),
                    key: key.clone(),
                    winner: winner.clone(),
                });
                continue;
            }
            owner.insert(key.clone(), contribution.plugin.clone());
            out.env.insert(key.clone(), value.clone());
        }

        let home = data_dir_for(&contribution.plugin);
        for entry in &contribution.path_prepends {
            match &home {
                Some(home) if is_within(home, entry) => out.path_prepends.push(entry.clone()),
                _ => out.rejections.push(Rejection::PathEscape {
                    plugin: contribution.plugin.clone(),
                    entry: entry.clone(),
                }),
            }
        }
    }

    out
}

/// Whether a key is refused outright.
///
/// Case-insensitive: Windows environment variables are case-insensitive, and a
/// case-sensitive check would let `Ld_Preload` through on a platform where it
/// still resolves.
pub fn is_denied(key: &str) -> bool {
    DENIED_ENV_KEYS
        .iter()
        .any(|denied| denied.eq_ignore_ascii_case(key))
}

/// Whether `entry` lies inside `root`.
///
/// Compared lexically after normalizing `.` and `..`, so a traversal cannot
/// pass by spelling. Deliberately does not canonicalize: this is pure policy
/// over a path that may not exist yet, and touching the filesystem here would
/// make the decision depend on state a plugin could race.
fn is_within(root: &Path, entry: &str) -> bool {
    let candidate = Path::new(entry);
    if !candidate.is_absolute() {
        return false;
    }
    let normalized = normalize(candidate);
    normalized.starts_with(normalize(root))
}

/// Resolve `.` and `..` lexically.
fn normalize(path: &Path) -> std::path::PathBuf {
    let mut out = std::path::PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn contribution(plugin: &str, env: &[(&str, &str)], paths: &[&str]) -> SpawnContribution {
        SpawnContribution {
            plugin: plugin.to_string(),
            env: env
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            path_prepends: paths.iter().map(|p| p.to_string()).collect(),
        }
    }

    fn homes(plugin: &str) -> Option<PathBuf> {
        Some(PathBuf::from(format!("/data/{plugin}")))
    }

    #[test]
    fn an_ordinary_variable_is_contributed() {
        let c = contribution("demo", &[("MY_TOKEN", "abc")], &[]);
        let out = resolve([&c], homes);
        assert_eq!(out.env.get("MY_TOKEN"), Some(&"abc".to_string()));
        assert!(out.rejections.is_empty());
    }

    #[test]
    fn every_code_execution_vector_is_refused() {
        for key in DENIED_ENV_KEYS {
            let c = contribution("evil", &[(key, "/tmp/pwn.so")], &[]);
            let out = resolve([&c], homes);
            assert!(out.env.is_empty(), "{key} must not reach a spawned session");
            assert_eq!(
                out.rejections,
                vec![Rejection::DeniedKey {
                    plugin: "evil".to_string(),
                    key: key.to_string(),
                }]
            );
        }
    }

    #[test]
    fn the_denylist_is_case_insensitive() {
        // Windows env vars are case-insensitive, so a case-sensitive check
        // would fail open there.
        for spelling in ["ld_preload", "Ld_PreLoad", "LD_PRELOAD"] {
            assert!(is_denied(spelling), "{spelling}");
        }
    }

    #[test]
    fn path_cannot_be_set_outright() {
        // Setting PATH wholesale would replace program lookup for everything
        // the agent runs — prepending under the plugin's own directory is the
        // only permitted shape.
        let c = contribution("demo", &[("PATH", "/tmp/evil")], &[]);
        let out = resolve([&c], homes);
        assert!(out.env.is_empty());
        assert!(matches!(out.rejections[0], Rejection::DeniedKey { .. }));
    }

    #[test]
    fn a_path_prepend_inside_the_plugins_directory_is_allowed() {
        let c = contribution("demo", &[], &["/data/demo/bin"]);
        let out = resolve([&c], homes);
        assert_eq!(out.path_prepends, vec!["/data/demo/bin".to_string()]);
        assert!(out.rejections.is_empty());
    }

    #[test]
    fn a_path_prepend_outside_the_plugins_directory_is_refused() {
        for escape in ["/usr/bin", "/data/other/bin", "/data/demo/../other/bin"] {
            let c = contribution("demo", &[], &[escape]);
            let out = resolve([&c], homes);
            assert!(
                out.path_prepends.is_empty(),
                "{escape} must not be prepended"
            );
            assert!(matches!(out.rejections[0], Rejection::PathEscape { .. }));
        }
    }

    #[test]
    fn a_relative_path_prepend_is_refused() {
        // A relative entry resolves against the agent's cwd, which the plugin
        // does not control.
        let c = contribution("demo", &[], &["bin"]);
        let out = resolve([&c], homes);
        assert!(out.path_prepends.is_empty());
    }

    #[test]
    fn a_plugin_with_no_data_directory_cannot_prepend() {
        let c = contribution("demo", &[], &["/data/demo/bin"]);
        let out = resolve([&c], |_| None);
        assert!(out.path_prepends.is_empty());
        assert!(matches!(out.rejections[0], Rejection::PathEscape { .. }));
    }

    #[test]
    fn contributions_are_append_only_and_first_wins() {
        let first = contribution("early", &[("SHARED", "one")], &[]);
        let second = contribution("late", &[("SHARED", "two")], &[]);
        let out = resolve([&first, &second], homes);

        assert_eq!(out.env.get("SHARED"), Some(&"one".to_string()));
        assert_eq!(
            out.rejections,
            vec![Rejection::AlreadySet {
                plugin: "late".to_string(),
                key: "SHARED".to_string(),
                winner: "early".to_string(),
            }]
        );
    }

    #[test]
    fn a_rejection_names_the_plugin_the_key_and_the_reason() {
        let c = contribution("sneaky", &[("LD_PRELOAD", "x")], &[]);
        let rendered = resolve([&c], homes).rejections[0].to_string();
        assert!(rendered.contains("sneaky"), "{rendered}");
        assert!(rendered.contains("LD_PRELOAD"), "{rendered}");
        assert!(rendered.contains("every agent session"), "{rendered}");
    }

    #[test]
    fn one_plugins_rejection_does_not_drop_anothers_contribution() {
        let bad = contribution("evil", &[("BASH_ENV", "/tmp/x")], &[]);
        let good = contribution("fine", &[("MY_TOKEN", "abc")], &[]);
        let out = resolve([&bad, &good], homes);

        assert_eq!(out.env.get("MY_TOKEN"), Some(&"abc".to_string()));
        assert_eq!(out.rejections.len(), 1);
    }

    #[test]
    fn nothing_is_contributed_when_no_plugin_asks() {
        let out = resolve([], homes);
        assert!(out.env.is_empty());
        assert!(out.path_prepends.is_empty());
        assert!(out.rejections.is_empty());
    }
}
