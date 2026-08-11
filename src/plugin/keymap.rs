//! Turning discovered manifests into keymap entries.
//!
//! Like [`super::spawn`] and [`super::commands`], this involves no VM: a
//! binding's address, its name and its default chord are manifest facts, so the
//! keymap knows a plugin's keys before any of its code has run — which is what
//! lets `thurbox-cli plugin doctor` report a dead key for a plugin that faults
//! at `init`.
//!
//! The keymap itself lives in `session::keybindings` and is compiled in every
//! build; only this translation is behind the plugin host.

use crate::session::keybindings::{KeyChord, PaneBindingDecl, PaneBindingId};
use crate::session::plugin_manifest::PluginManifest;

use super::discovery::DiscoveredPlugin;

/// What one manifest contributes, given whether its view half was granted the
/// capability to receive input.
///
/// A plugin without that grant contributes **nothing**: a chord that resolved to
/// a plugin the host would then refuse to deliver to would be a key that does
/// nothing, which is the failure the manifest's own validation exists to prevent.
///
/// A chord the grammar does not accept is dropped rather than refused here.
/// Manifest validation already rejects one, so reaching this is a host bug rather
/// than an author's mistake, and the useful behaviour for a bug is an unbound
/// binding the editor can still bind.
pub fn pane_bindings_for(manifest: &PluginManifest, input_granted: bool) -> Vec<PaneBindingDecl> {
    if !input_granted {
        return Vec::new();
    }
    manifest
        .keybindings
        .iter()
        .map(|binding| PaneBindingDecl {
            id: PaneBindingId::new(&manifest.name, &binding.pane, &binding.id),
            title: binding.display_title().to_string(),
            chord: binding.chord.as_deref().and_then(KeyChord::parse),
        })
        .collect()
}

/// Every declaration a set of discovered plugins contributes, in discovery
/// order.
///
/// Used where no host is running — `plugin doctor` — so the grant is read from
/// the manifest's own request. The lifecycle uses each slot's *granted* set
/// instead, since that is what will actually be enforced at delivery.
pub fn declarations_for<'a>(
    plugins: impl IntoIterator<Item = &'a DiscoveredPlugin>,
) -> Vec<PaneBindingDecl> {
    plugins
        .into_iter()
        .flat_map(|p| {
            let granted = p
                .manifest
                .capabilities
                .contains(&crate::session::plugin_manifest::Capability::Input);
            pane_bindings_for(&p.manifest, granted)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::discovery::discover_in;
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

    const PANE_WITH_KEYS: &str = r#"
capabilities = ["render", "input"]
[[panes]]
id = "board"
[[keybindings]]
id = "next"
pane = "board"
title = "Next row"
chord = "j"
[[keybindings]]
id = "unbound"
pane = "board"
"#;

    fn declared(root: &Path) -> Vec<PaneBindingDecl> {
        declarations_for(discover_in(&[], Some(root)).loadable.iter())
    }

    #[test]
    fn a_declared_binding_becomes_an_entry_without_a_vm() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "notes", PANE_WITH_KEYS);

        let decls = declared(tmp.path());
        assert_eq!(decls.len(), 2);
        assert_eq!(decls[0].id, PaneBindingId::new("notes", "board", "next"));
        assert_eq!(decls[0].title, "Next row");
        assert_eq!(decls[0].chord, KeyChord::parse("j"));
    }

    #[test]
    fn a_binding_with_no_chord_is_declared_unbound() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "notes", PANE_WITH_KEYS);

        let decls = declared(tmp.path());
        let unbound = decls
            .iter()
            .find(|d| d.id.id == "unbound")
            .expect("declared");
        assert_eq!(unbound.chord, None);
        // The title falls back to the id, so the editor still has a row label.
        assert_eq!(unbound.title, "unbound");
    }

    #[test]
    fn a_plugin_without_the_input_grant_contributes_nothing() {
        let manifest = PluginManifest::from_toml(
            Path::new("/plugins/notes/plugin.toml"),
            &format!("name = \"notes\"\napi_version = 1\n{PANE_WITH_KEYS}"),
        )
        .expect("valid");

        assert!(pane_bindings_for(&manifest, false).is_empty());
        assert_eq!(pane_bindings_for(&manifest, true).len(), 2);
    }

    #[test]
    fn a_plugin_declaring_no_keybinding_contributes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "quiet",
            "capabilities = [\"render\", \"input\"]\n[[panes]]\nid = \"main\"\n",
        );
        assert!(declared(tmp.path()).is_empty());
    }
}
