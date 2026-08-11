//! No bundled pane is on screen before a user asks for it.
//!
//! `PaneDecl::default_visible` seeds to `true`, which is right for a plugin an
//! author installed on purpose and wrong for one that arrives inside the binary.
//! While `plugins` was outside the default feature set the distinction was
//! invisible — no installed binary ran a bundled plugin at all. Since the host
//! joined the default feature set (ADR-40) an omitted seed puts a pane in every
//! fresh install's right column, and today every bundled pane is either a
//! *reproduction* of a native pane the interface still draws or a worked example.
//! Either way, showing it means shipping a surface nobody asked for — two session
//! lists side by side, or a "Hello" demo.
//!
//! So the rule binds the whole bundled set rather than the manifests that
//! happened to remember it, and [`PANES_DRAWN_IN_A_NATIVE_PANES_PLACE`] is where
//! the first handover argues its exception.
//!
//! Like `tests/teardown_gate.rs`, the manifests are read from the source tree
//! rather than through `plugin::discovery`, so the check runs and means the same
//! thing with or without the `plugins` Cargo feature — the property is about the
//! bytes that ship, not about how this test binary was built. It also means a new
//! bundled plugin is covered the moment its directory exists.

use std::fs;
use std::path::{Path, PathBuf};

use thurbox::session::plugin_manifest::PluginManifest;

/// Bundled panes that may seed visible, as `(plugin, pane)`.
///
/// Empty, because no pane has been handed over: every bundled plugin renders
/// beside the native pane it reproduces. A handover adds its pane here in the
/// same change that stops `src/app/view.rs` drawing the native one — which is the
/// point of the list. A pane that is visible *and* duplicated is the mistake, and
/// the two facts live in one commit.
const PANES_DRAWN_IN_A_NATIVE_PANES_PLACE: &[(&str, &str)] = &[];

fn bundled_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/plugin/bundled")
}

/// Every bundled plugin directory, by name, sorted so a failure reads the same
/// way twice.
fn bundled_plugins() -> Vec<(String, PathBuf)> {
    let dir = bundled_dir();
    let mut found: Vec<(String, PathBuf)> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.join("plugin.toml").is_file())
        .map(|p| {
            let name = p
                .file_name()
                .expect("bundled plugin dir has a name")
                .to_string_lossy()
                .into_owned();
            (name, p)
        })
        .collect();
    found.sort();
    found
}

fn manifest_at(dir: &Path) -> PluginManifest {
    let path = dir.join("plugin.toml");
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    PluginManifest::from_toml(&path, &text)
        .unwrap_or_else(|e| panic!("{} is not a valid manifest: {e}", path.display()))
}

#[test]
fn every_bundled_pane_seeds_hidden() {
    let mut visible = Vec::new();
    for (dir_name, dir) in bundled_plugins() {
        let manifest = manifest_at(&dir);
        for pane in &manifest.panes {
            let exempt = PANES_DRAWN_IN_A_NATIVE_PANES_PLACE
                .iter()
                .any(|(plugin, id)| *plugin == manifest.name && *id == pane.id);
            if pane.default_visible && !exempt {
                visible.push(format!("{dir_name}/plugin.toml: pane `{}`", pane.id));
            }
        }
    }

    assert!(
        visible.is_empty(),
        "a bundled pane seeds visible, so it would open in every fresh install:\n  {}\n\
         `default_visible` defaults to true — say `default_visible = false`, or, if \
         this pane is now drawn in a native pane's place, add it to \
         PANES_DRAWN_IN_A_NATIVE_PANES_PLACE in the change that stops drawing the \
         native one.",
        visible.join("\n  ")
    );
}

/// The check is only worth having while there is something to check: an empty
/// bundled set, or a set whose manifests declare no panes, would pass the rule
/// above while proving nothing.
#[test]
fn the_bundled_set_declares_panes_to_check() {
    let plugins = bundled_plugins();
    assert!(
        !plugins.is_empty(),
        "no bundled plugin directories found under {}",
        bundled_dir().display()
    );
    let panes: usize = plugins
        .iter()
        .map(|(_, dir)| manifest_at(dir).panes.len())
        .sum();
    assert!(
        panes >= plugins.len(),
        "{} bundled plugins declare only {panes} panes between them — if a plugin \
         legitimately has none, this count is what needs revisiting",
        plugins.len()
    );
}
