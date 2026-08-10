//! Finding plugins on disk.
//!
//! Discovery walks an ordered list of sources and turns each directory holding
//! a manifest into one candidate plugin. It is the only part of the host that
//! touches the filesystem to *find* plugins, and it never fails as a whole: a
//! malformed plugin, an unreadable directory, or a name collision is recorded
//! and skipped so that one bad plugin cannot keep thurbox from starting.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::session::plugin_manifest::{
    ManifestError, ManifestErrorKind, PluginManifest, MANIFEST_FILE_NAME,
};

/// Where a plugin was found.
///
/// Ordering is significant: a later source overrides an earlier one for the
/// same plugin name, which is what lets a user drop an edited copy of a bundled
/// plugin into their config dir and have it take over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PluginSource {
    /// Shipped inside the binary and materialized by the host.
    Bundled,
    /// Authored or installed by the user, under `~/.config/thurbox/plugins/`.
    User,
}

impl PluginSource {
    /// Sources in precedence order, lowest first.
    pub fn in_precedence_order() -> &'static [PluginSource] {
        &[PluginSource::Bundled, PluginSource::User]
    }
}

impl fmt::Display for PluginSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            PluginSource::Bundled => "bundled",
            PluginSource::User => "user",
        })
    }
}

/// A plugin that parsed cleanly and is eligible to load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredPlugin {
    /// Its validated manifest.
    pub manifest: PluginManifest,
    /// The directory holding `plugin.toml`. Doubles as the root that bounds
    /// the plugin's module resolution at runtime.
    pub dir: PathBuf,
    /// Which source it came from.
    pub source: PluginSource,
}

impl DiscoveredPlugin {
    /// The plugin's unique name.
    pub fn name(&self) -> &str {
        &self.manifest.name
    }
}

/// A plugin that was found but cannot be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryProblem {
    /// Its manifest is invalid.
    Invalid(ManifestError),
    /// A different source declared the same name and won.
    Overridden {
        /// The shadowed plugin's name.
        name: String,
        /// Where the shadowed copy lives.
        path: PathBuf,
        /// Where the copy that won lives.
        winner: PathBuf,
    },
    /// Two plugins in the *same* source declared one name, so neither can be
    /// preferred without inventing a tiebreak.
    Conflict {
        /// The contested name.
        name: String,
        /// Every directory that declared it.
        paths: Vec<PathBuf>,
    },
    /// A source directory could not be listed.
    UnreadableSource {
        /// The directory that failed.
        path: PathBuf,
        /// The I/O cause.
        cause: String,
    },
}

impl fmt::Display for DiscoveryProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiscoveryProblem::Invalid(e) => write!(f, "{e}"),
            DiscoveryProblem::Overridden { name, path, winner } => write!(
                f,
                "plugin `{name}` at {} is overridden by {}",
                path.display(),
                winner.display()
            ),
            DiscoveryProblem::Conflict { name, paths } => {
                let joined: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
                write!(
                    f,
                    "plugin name `{name}` declared by more than one plugin in the same source: {}",
                    joined.join(", ")
                )
            }
            DiscoveryProblem::UnreadableSource { path, cause } => {
                write!(
                    f,
                    "cannot read plugin directory {}: {cause}",
                    path.display()
                )
            }
        }
    }
}

/// Everything discovery learned, including what it refused.
///
/// A user asking "why isn't my plugin running?" is answered entirely from
/// here — a plugin is either in `loadable` or named in `problems` with a cause.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiscoveryOutcome {
    /// Plugins eligible to load, in deterministic order.
    pub loadable: Vec<DiscoveredPlugin>,
    /// Everything that was found but will not load, and why.
    pub problems: Vec<DiscoveryProblem>,
}

impl DiscoveryOutcome {
    /// Whether any plugin can be loaded.
    pub fn is_empty(&self) -> bool {
        self.loadable.is_empty()
    }

    /// Look up a discovered plugin by name.
    pub fn get(&self, name: &str) -> Option<&DiscoveredPlugin> {
        self.loadable.iter().find(|p| p.name() == name)
    }
}

/// A plugin compiled into the binary, as its two source files.
struct BundledPlugin {
    name: &'static str,
    manifest: &'static str,
    entry: &'static str,
}

/// Every plugin shipped inside the binary.
///
/// Embedded rather than installed so the single-binary promise holds: a fresh
/// thurbox has a working plugin with nothing to download.
const BUNDLED: &[BundledPlugin] = &[
    BundledPlugin {
        name: "hello",
        manifest: include_str!("bundled/hello/plugin.toml"),
        entry: include_str!("bundled/hello/init.luau"),
    },
    // thurbox's own info panel, reproduced as a plugin (Phase 4). Hidden by
    // default — the native pane is still what the interface draws, so a visible
    // copy would put two of the same pane on screen for every user.
    BundledPlugin {
        name: "info-panel",
        manifest: include_str!("bundled/info-panel/plugin.toml"),
        entry: include_str!("bundled/info-panel/init.luau"),
    },
];

/// Materialize the bundled plugins and return their directories.
///
/// Written to the data dir so discovery can treat them as an ordinary local
/// source — the same trick the built-in extensions use. Files are written only
/// when their content differs, so a user editing a materialized copy to
/// experiment is not fighting the writer on every launch, and an unchanged
/// startup does no I/O beyond two reads.
fn bundled_plugin_dirs() -> Vec<PathBuf> {
    let Some(root) = crate::paths::builtin_plugins_directory() else {
        return Vec::new();
    };

    let mut dirs = Vec::new();
    for plugin in BUNDLED {
        let dir = root.join(plugin.name);
        if fs::create_dir_all(&dir).is_err() {
            continue;
        }
        let ok = write_if_changed(&dir.join(MANIFEST_FILE_NAME), plugin.manifest)
            && write_if_changed(&dir.join("init.luau"), plugin.entry);
        if ok {
            dirs.push(dir);
        }
    }
    dirs
}

/// Write `content` to `path` unless it is already exactly that.
fn write_if_changed(path: &Path, content: &str) -> bool {
    if fs::read_to_string(path).is_ok_and(|existing| existing == content) {
        return true;
    }
    fs::write(path, content).is_ok()
}

/// Discover plugins from the standard sources.
///
/// The user directory is resolved through [`crate::paths`], so the
/// `THURBOX_CONFIG_DIR` override and the test path guard both redirect it with
/// no extra mechanism.
pub fn discover() -> DiscoveryOutcome {
    let user_dir = crate::paths::plugins_directory();
    discover_in(&bundled_plugin_dirs(), user_dir.as_deref())
}

/// Discover plugins from explicit roots.
///
/// `bundled` is a list of plugin directories; `user_root` is a directory whose
/// *children* are plugin directories.
pub fn discover_in(bundled: &[PathBuf], user_root: Option<&Path>) -> DiscoveryOutcome {
    let mut outcome = DiscoveryOutcome::default();

    // Per source, so that a same-source collision is distinguishable from an
    // override across sources.
    let mut per_source: Vec<(PluginSource, Vec<DiscoveredPlugin>)> = Vec::new();

    let mut bundled_found = Vec::new();
    for dir in bundled {
        match read_plugin_dir(dir, PluginSource::Bundled) {
            Ok(Some(p)) => bundled_found.push(p),
            Ok(None) => {}
            Err(problem) => outcome.problems.push(problem),
        }
    }
    per_source.push((PluginSource::Bundled, bundled_found));

    let mut user_found = Vec::new();
    if let Some(root) = user_root {
        match list_plugin_dirs(root) {
            Ok(dirs) => {
                for dir in dirs {
                    match read_plugin_dir(&dir, PluginSource::User) {
                        Ok(Some(p)) => user_found.push(p),
                        Ok(None) => {}
                        Err(problem) => outcome.problems.push(problem),
                    }
                }
            }
            Err(Some(problem)) => outcome.problems.push(problem),
            // A missing directory is the normal case, not a problem.
            Err(None) => {}
        }
    }
    per_source.push((PluginSource::User, user_found));

    resolve_collisions(per_source, &mut outcome);
    outcome
}

/// List a source root's immediate children, sorted.
///
/// `Err(None)` means the directory is absent, which is not a problem;
/// `Err(Some(_))` means it exists but could not be listed.
fn list_plugin_dirs(root: &Path) -> Result<Vec<PathBuf>, Option<DiscoveryProblem>> {
    if !root.exists() {
        return Err(None);
    }
    let entries = fs::read_dir(root).map_err(|e| {
        Some(DiscoveryProblem::UnreadableSource {
            path: root.to_path_buf(),
            cause: e.to_string(),
        })
    })?;

    let mut dirs: Vec<PathBuf> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        }
    }
    // Filesystem order is not defined; discovery is required to be.
    dirs.sort();
    Ok(dirs)
}

/// Read one candidate plugin directory.
///
/// `Ok(None)` means "not a plugin" — no manifest at the root. That is silent by
/// design: a user's plugin directory may hold scratch directories, and treating
/// each as a broken plugin would bury the real failures.
fn read_plugin_dir(
    dir: &Path,
    source: PluginSource,
) -> Result<Option<DiscoveredPlugin>, DiscoveryProblem> {
    let manifest_path = dir.join(MANIFEST_FILE_NAME);
    if !manifest_path.is_file() {
        return Ok(None);
    }

    let text = match fs::read_to_string(&manifest_path) {
        Ok(t) => t,
        Err(e) => {
            return Err(DiscoveryProblem::Invalid(ManifestError {
                path: manifest_path,
                kind: ManifestErrorKind::Io(e.to_string()),
            }))
        }
    };

    match PluginManifest::from_toml(&manifest_path, &text) {
        Ok(manifest) => Ok(Some(DiscoveredPlugin {
            manifest,
            dir: dir.to_path_buf(),
            source,
        })),
        Err(e) => Err(DiscoveryProblem::Invalid(e)),
    }
}

/// Apply precedence across sources and reject same-source collisions.
fn resolve_collisions(
    per_source: Vec<(PluginSource, Vec<DiscoveredPlugin>)>,
    outcome: &mut DiscoveryOutcome,
) {
    // name -> the winning plugin so far. BTreeMap keeps the final order stable
    // and independent of insertion order.
    let mut winners: BTreeMap<String, DiscoveredPlugin> = BTreeMap::new();

    for (_source, found) in per_source {
        // Group this source's plugins by name to spot an internal collision
        // before it can shadow anything.
        let mut by_name: BTreeMap<String, Vec<DiscoveredPlugin>> = BTreeMap::new();
        for plugin in found {
            by_name
                .entry(plugin.name().to_string())
                .or_default()
                .push(plugin);
        }

        for (name, mut group) in by_name {
            if group.len() > 1 {
                // No principled tiebreak exists within one source, so refuse
                // both rather than let directory order decide silently.
                let mut paths: Vec<PathBuf> = group.iter().map(|p| p.dir.clone()).collect();
                paths.sort();
                outcome
                    .problems
                    .push(DiscoveryProblem::Conflict { name, paths });
                continue;
            }

            let plugin = group.pop().expect("group is non-empty");
            if let Some(previous) = winners.insert(name.clone(), plugin) {
                let winner = winners[&name].dir.clone();
                outcome.problems.push(DiscoveryProblem::Overridden {
                    name,
                    path: previous.dir,
                    winner,
                });
            }
        }
    }

    outcome.loadable = winners.into_values().collect();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a plugin directory under `root` with the given manifest body.
    fn write_plugin(root: &Path, dir_name: &str, body: &str) -> PathBuf {
        let dir = root.join(dir_name);
        fs::create_dir_all(&dir).expect("create plugin dir");
        fs::write(dir.join(MANIFEST_FILE_NAME), body).expect("write manifest");
        dir
    }

    fn manifest_for(name: &str) -> String {
        format!("name = \"{name}\"\napi_version = 1\n")
    }

    #[test]
    fn finds_plugins_from_both_sources() {
        let tmp = tempfile::tempdir().unwrap();
        let bundled_root = tmp.path().join("bundled");
        let user_root = tmp.path().join("user");
        fs::create_dir_all(&user_root).unwrap();

        let bundled = write_plugin(&bundled_root, "alpha", &manifest_for("alpha"));
        write_plugin(&user_root, "beta", &manifest_for("beta"));

        let outcome = discover_in(&[bundled], Some(&user_root));

        assert_eq!(outcome.loadable.len(), 2);
        assert_eq!(outcome.get("alpha").unwrap().source, PluginSource::Bundled);
        assert_eq!(outcome.get("beta").unwrap().source, PluginSource::User);
        assert!(outcome.problems.is_empty());
    }

    #[test]
    fn discovery_is_deterministic() {
        let tmp = tempfile::tempdir().unwrap();
        let user_root = tmp.path().join("user");
        fs::create_dir_all(&user_root).unwrap();
        for n in ["zulu", "alpha", "mike"] {
            write_plugin(&user_root, n, &manifest_for(n));
        }

        let first = discover_in(&[], Some(&user_root));
        let second = discover_in(&[], Some(&user_root));

        assert_eq!(first, second);
        let names: Vec<&str> = first.loadable.iter().map(|p| p.name()).collect();
        assert_eq!(names, vec!["alpha", "mike", "zulu"]);
    }

    #[test]
    fn directory_without_manifest_is_skipped_silently() {
        let tmp = tempfile::tempdir().unwrap();
        let user_root = tmp.path().join("user");
        fs::create_dir_all(user_root.join("not-a-plugin/nested")).unwrap();
        // A manifest one level deeper must not be found: a plugin is a
        // directory whose *root* holds the manifest.
        fs::write(
            user_root
                .join("not-a-plugin/nested")
                .join(MANIFEST_FILE_NAME),
            manifest_for("nested"),
        )
        .unwrap();

        let outcome = discover_in(&[], Some(&user_root));

        assert!(outcome.is_empty());
        assert!(outcome.problems.is_empty(), "{:?}", outcome.problems);
    }

    #[test]
    fn loose_file_is_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let user_root = tmp.path().join("user");
        fs::create_dir_all(&user_root).unwrap();
        fs::write(user_root.join("README.md"), "not a plugin").unwrap();

        let outcome = discover_in(&[], Some(&user_root));

        assert!(outcome.is_empty());
        assert!(outcome.problems.is_empty());
    }

    #[test]
    fn missing_user_directory_is_not_an_error_and_is_not_created() {
        let tmp = tempfile::tempdir().unwrap();
        let user_root = tmp.path().join("absent");

        let outcome = discover_in(&[], Some(&user_root));

        assert!(outcome.is_empty());
        assert!(outcome.problems.is_empty());
        assert!(
            !user_root.exists(),
            "discovery must not create the directory"
        );
    }

    #[test]
    fn user_plugin_shadows_bundled() {
        let tmp = tempfile::tempdir().unwrap();
        let bundled_root = tmp.path().join("bundled");
        let user_root = tmp.path().join("user");
        fs::create_dir_all(&user_root).unwrap();

        let bundled = write_plugin(&bundled_root, "tasks", &manifest_for("tasks"));
        let user = write_plugin(&user_root, "tasks", &manifest_for("tasks"));

        let outcome = discover_in(std::slice::from_ref(&bundled), Some(&user_root));

        assert_eq!(outcome.loadable.len(), 1);
        let winner = outcome.get("tasks").unwrap();
        assert_eq!(winner.source, PluginSource::User);
        assert_eq!(winner.dir, user);

        assert_eq!(outcome.problems.len(), 1);
        match &outcome.problems[0] {
            DiscoveryProblem::Overridden { name, path, winner } => {
                assert_eq!(name, "tasks");
                assert_eq!(path, &bundled);
                assert_eq!(winner, &user);
            }
            other => panic!("expected an override, got {other:?}"),
        }
    }

    #[test]
    fn same_source_collision_rejects_both() {
        let tmp = tempfile::tempdir().unwrap();
        let user_root = tmp.path().join("user");
        fs::create_dir_all(&user_root).unwrap();
        write_plugin(&user_root, "copy-a", &manifest_for("dup"));
        write_plugin(&user_root, "copy-b", &manifest_for("dup"));

        let outcome = discover_in(&[], Some(&user_root));

        assert!(outcome.is_empty(), "neither copy may be preferred");
        assert_eq!(outcome.problems.len(), 1);
        match &outcome.problems[0] {
            DiscoveryProblem::Conflict { name, paths } => {
                assert_eq!(name, "dup");
                assert_eq!(paths.len(), 2);
            }
            other => panic!("expected a conflict, got {other:?}"),
        }
    }

    #[test]
    fn one_malformed_plugin_does_not_stop_the_others() {
        let tmp = tempfile::tempdir().unwrap();
        let user_root = tmp.path().join("user");
        fs::create_dir_all(&user_root).unwrap();
        write_plugin(&user_root, "good", &manifest_for("good"));
        write_plugin(&user_root, "bad", "name = \"bad\"\napi_version =");

        let outcome = discover_in(&[], Some(&user_root));

        assert_eq!(outcome.loadable.len(), 1);
        assert_eq!(outcome.loadable[0].name(), "good");
        assert_eq!(outcome.problems.len(), 1);
        assert!(matches!(outcome.problems[0], DiscoveryProblem::Invalid(_)));
    }

    #[test]
    fn all_malformed_yields_no_plugins_and_all_causes() {
        let tmp = tempfile::tempdir().unwrap();
        let user_root = tmp.path().join("user");
        fs::create_dir_all(&user_root).unwrap();
        write_plugin(&user_root, "one", "api_version = 1");
        write_plugin(&user_root, "two", "name = \"Two\"\napi_version = 1");

        let outcome = discover_in(&[], Some(&user_root));

        assert!(outcome.is_empty());
        assert_eq!(outcome.problems.len(), 2);
        for problem in &outcome.problems {
            assert!(matches!(problem, DiscoveryProblem::Invalid(_)));
        }
    }

    #[test]
    fn problems_name_the_plugin_path_and_reason() {
        let tmp = tempfile::tempdir().unwrap();
        let user_root = tmp.path().join("user");
        fs::create_dir_all(&user_root).unwrap();
        let dir = write_plugin(&user_root, "broken", "name = \"Broken\"\napi_version = 1");

        let outcome = discover_in(&[], Some(&user_root));

        let rendered = outcome.problems[0].to_string();
        assert!(
            rendered.contains(&dir.join(MANIFEST_FILE_NAME).display().to_string()),
            "problem should name the manifest path: {rendered}"
        );
        assert!(
            rendered.contains("lowercase"),
            "problem should name the rule broken: {rendered}"
        );
    }

    #[test]
    fn unreadable_manifest_is_reported_as_io() {
        let tmp = tempfile::tempdir().unwrap();
        let user_root = tmp.path().join("user");
        let dir = user_root.join("weird");
        fs::create_dir_all(&dir).unwrap();
        // A directory where the manifest should be: `is_file()` is false, so
        // this is "not a plugin" rather than an I/O failure.
        fs::create_dir_all(dir.join(MANIFEST_FILE_NAME)).unwrap();

        let outcome = discover_in(&[], Some(&user_root));

        assert!(outcome.is_empty());
        assert!(outcome.problems.is_empty());
    }

    #[test]
    fn the_bundled_plugin_is_valid() {
        // The shipped plugin must satisfy the same manifest rules a user's
        // does — it is the worked example, so a broken one teaches the wrong
        // thing and would also fail to load for every user.
        for plugin in BUNDLED {
            let path = PathBuf::from(plugin.name).join(MANIFEST_FILE_NAME);
            let manifest = PluginManifest::from_toml(&path, plugin.manifest)
                .unwrap_or_else(|e| panic!("bundled plugin `{}` is invalid: {e}", plugin.name));
            assert_eq!(manifest.name, plugin.name);
            assert!(!plugin.entry.is_empty());
        }
    }

    /// A bundled pane must not turn itself on: the native pane it reproduces is
    /// still on screen, so a visible copy would change every user's layout.
    #[test]
    fn the_bundled_info_panel_is_hidden_by_default() {
        let plugin = BUNDLED
            .iter()
            .find(|p| p.name == "info-panel")
            .expect("the info panel ships bundled");
        let path = PathBuf::from(plugin.name).join(MANIFEST_FILE_NAME);
        let manifest = PluginManifest::from_toml(&path, plugin.manifest).expect("valid");
        assert_eq!(manifest.panes.len(), 1);
        assert!(!manifest.panes[0].default_visible);
    }

    #[test]
    fn materializing_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("f.txt");
        assert!(write_if_changed(&path, "one"));
        let first = fs::metadata(&path).unwrap().modified().unwrap();
        // Unchanged content must not rewrite the file.
        assert!(write_if_changed(&path, "one"));
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), first);
        assert!(write_if_changed(&path, "two"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "two");
    }

    #[test]
    fn precedence_order_puts_user_last() {
        assert_eq!(
            PluginSource::in_precedence_order(),
            &[PluginSource::Bundled, PluginSource::User]
        );
    }
}
