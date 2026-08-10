//! Turning discovered manifests into the spawn-contribution registry.
//!
//! This is the whole plugin-side of spawn contributions: no VM is involved,
//! because the contribution is static manifest data. That is deliberate — the
//! spawn environment is finalized on the UI thread for a TUI spawn, so a
//! per-spawn Luau callback would put plugin code on the render loop. Reading it
//! from the manifest means the spawn path only ever reads a snapshot.
//!
//! Publishing is one-way and idempotent: discovery produces a registry, the
//! registry replaces whatever was published before.

use crate::session::plugin_manifest::Capability;
use crate::session::spawn_contribution::{Registry, SpawnContribution};

use super::discovery::DiscoveredPlugin;

/// Build the registry for a set of discovered plugins.
///
/// A plugin contributes only if it declared both a `[spawn]` table and the
/// `spawn` capability. The capability is redundant here — manifest validation
/// already rejects one without the other — but checking it at the point of use
/// keeps the grant load-bearing rather than decorative, so a future manifest
/// path that skipped validation could not quietly widen a plugin's reach.
pub fn registry_for<'a>(plugins: impl IntoIterator<Item = &'a DiscoveredPlugin>) -> Registry {
    let mut registry = Registry::default();
    for plugin in plugins {
        let Some(decl) = plugin.manifest.spawn.as_ref() else {
            continue;
        };
        if !plugin.manifest.grants(Capability::Spawn) {
            continue;
        }
        registry.insert(
            plugin.dir.clone(),
            SpawnContribution {
                plugin: plugin.name().to_string(),
                env: decl.env.clone(),
                // Empty, and not because nothing was declared: `PATH` prepends
                // have no manifest surface, since tmux replaces a pane's
                // `PATH` with the server's own and would drop one silently.
                // The policy layer still carries the rule for the day a
                // backend can deliver it.
                path_prepends: Vec::new(),
            },
        );
    }
    registry
}

/// Discover plugins and publish their contributions for this process.
///
/// Called once by each binary that can spawn a session. Discovery is a
/// filesystem walk, so it happens here, at startup, rather than on the spawn
/// path.
pub fn publish_from_discovery() {
    let outcome = super::discovery::discover();
    crate::session::spawn_contribution::publish(registry_for(outcome.loadable.iter()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::discovery::discover_in;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;

    fn write_plugin(root: &Path, name: &str, manifest_extra: &str) {
        let dir = root.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("plugin.toml"),
            format!("name = \"{name}\"\napi_version = 1\n{manifest_extra}"),
        )
        .unwrap();
        fs::write(dir.join("init.luau"), "return {}").unwrap();
    }

    fn registry_over(root: &Path) -> Registry {
        registry_for(discover_in(&[], Some(root)).loadable.iter())
    }

    #[test]
    fn a_declared_contribution_is_published() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "ci",
            "capabilities = [\"spawn\"]\n[spawn.env]\nCI_TOKEN_FILE = \"/run/secrets/ci\"\n",
        );

        let out = registry_over(tmp.path()).resolve(&BTreeSet::new());
        assert_eq!(
            out.env.get("CI_TOKEN_FILE"),
            Some(&"/run/secrets/ci".to_string())
        );
        assert!(out.rejections.is_empty());
    }

    #[test]
    fn a_plugin_declaring_nothing_publishes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "quiet", "capabilities = [\"log\"]\n");
        assert!(registry_over(tmp.path()).is_empty());
    }

    #[test]
    fn a_contribution_never_carries_a_path_prepend() {
        // There is no manifest surface for one, so nothing published can hold
        // a value the backend would drop on the floor.
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "ci",
            "capabilities = [\"spawn\"]\n[spawn.env]\nA = \"1\"\n",
        );

        let out = registry_over(tmp.path()).resolve(&BTreeSet::new());
        assert!(out.path_prepends.is_empty());
        assert!(out.rejections.is_empty());
    }

    #[test]
    fn publishing_does_not_execute_plugin_code() {
        // Every entry file here is broken; a registry is still produced,
        // because building one never compiles a plugin.
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "ci",
            "capabilities = [\"spawn\"]\n[spawn.env]\nA = \"1\"\n",
        );
        fs::write(tmp.path().join("ci").join("init.luau"), "return {").unwrap();

        let registry = registry_over(tmp.path());
        assert!(!registry.is_empty());
    }
}
