//! No bundled pane is on screen before a user asks for it.
//!
//! `PaneDecl::default_visible` seeds to `true`, which is right for a plugin an
//! author installed on purpose and wrong for one that arrives inside the binary.
//! While `plugins` was outside the default feature set the distinction was
//! invisible — no installed binary ran a bundled plugin at all. Since the host
//! joined the default feature set (ADR-40) an omitted seed puts a pane in every
//! fresh install's right column, and every bundled pane bar one is either a
//! *reproduction* of a native pane the interface still draws or a worked example.
//! Either way, showing it means shipping a surface nobody asked for — two session
//! lists side by side, or a "Hello" demo.
//!
//! So the rule binds the whole bundled set rather than the manifests that
//! happened to remember it, and [`PANES_DRAWN_IN_A_NATIVE_PANES_PLACE`] is where a
//! handover argues its exception. Three of its four entries still seed hidden, because
//! the kernel flags they replaced initialised to `false`; the other replaced a band that
//! was **always on screen**, so it seeds visible. Which value is right is not this
//! rule's to choose — [`a_handed_over_pane_seeds_at_the_native_panes_default`] compares
//! against the native default the list records.
//!
//! Like `tests/teardown_gate.rs`, the manifests are read from the source tree
//! rather than through `plugin::discovery`, so the check runs and means the same
//! thing with or without the `plugins` Cargo feature — the property is about the
//! bytes that ship, not about how this test binary was built. It also means a new
//! bundled plugin is covered the moment its directory exists.

use std::fs;
use std::path::{Path, PathBuf};

use thurbox::session::plugin_manifest::PluginManifest;

/// Bundled panes drawn in a native pane's place, as `(plugin, pane)` — the panes
/// this rule does not bind.
///
/// A handover adds its pane here in the same change that stops `src/app/view.rs`
/// drawing the native one, which is the point of the list: a pane that is visible
/// *and* duplicated is the mistake, and the two facts live in one commit.
///
/// The exemption **permits** a visible seed; it does not ask for one. A handover changes
/// which code draws a pane, not whether the pane is on screen — so each entry records the
/// visibility its native counterpart defaulted to, and that is what
/// [`a_handed_over_pane_seeds_at_the_native_panes_default`] compares against. The info
/// panel and the tasks pane replaced `App::show_info_panel` / `show_tasks_panel`, both
/// initialised to `false`; the automations band had no flag and no toggle at all, because
/// it was always on screen while `[features] automations` was on (ADR-56).
///
/// So the list is also the answer to "which bundled panes are no longer reproductions",
/// which is why an entry earns its place even when the seed did not change.
const PANES_DRAWN_IN_A_NATIVE_PANES_PLACE: &[HandedOver] = &[
    HandedOver::hidden("info-panel", "info"),
    HandedOver::hidden("tasks", "tasks"),
    HandedOver::visible("automations", "automations"),
    HandedOver::hidden("file-viewer", "files"),
];

/// One handed-over pane: which plugin and pane id, and whether the native pane it
/// replaced was on screen by default.
struct HandedOver {
    plugin: &'static str,
    pane: &'static str,
    /// The native pane's default visibility, which the replacement must seed at.
    native_default_visible: bool,
}

impl HandedOver {
    /// A pane replacing one that was hidden until an action showed it.
    const fn hidden(plugin: &'static str, pane: &'static str) -> Self {
        Self {
            plugin,
            pane,
            native_default_visible: false,
        }
    }

    /// A pane replacing one that was always on screen while its feature was on.
    const fn visible(plugin: &'static str, pane: &'static str) -> Self {
        Self {
            plugin,
            pane,
            native_default_visible: true,
        }
    }
}

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
                .any(|h| h.plugin == manifest.name && h.pane == pane.id);
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

/// A handed-over pane seeds at the visibility the native pane defaulted to.
///
/// The exemption above turns the rule off for such a pane, so without this nothing
/// would constrain its seed at all — and the harm a visible seed does is the same
/// one the rule exists for, reached by a different route: a column on every wide
/// install that nobody asked for, with the duplication that made a reproduction
/// obviously wrong now absent.
///
/// It compares against the **native default** each entry records rather than requiring
/// one particular value (ADR-56). Asserting `!default_visible` was right while every
/// handed-over pane replaced a hidden one, and it is not a weaker rule to generalise it:
/// this direction also catches a pane that seeds *hidden* while the band it replaced was
/// always on screen, which the old form silently permitted — and which would remove a
/// pane from every install as surely as the case it was written for adds one.
///
/// The second half follows from the first rather than being a separate rule: a pane that
/// seeds hidden must bind an action, or nothing could ever reveal it. A pane that seeds
/// visible needs none — and must not invent one, since a native pane that was always on
/// screen had no toggle whose meaning the replacement could take over.
#[test]
fn a_handed_over_pane_seeds_at_the_native_panes_default() {
    for handed in PANES_DRAWN_IN_A_NATIVE_PANES_PLACE {
        let manifest = manifest_at(&bundled_dir().join(handed.plugin));
        let pane = manifest
            .panes
            .iter()
            .find(|p| p.id == handed.pane)
            .unwrap_or_else(|| panic!("{} declares no pane `{}`", handed.plugin, handed.pane));
        assert_eq!(
            pane.default_visible,
            handed.native_default_visible,
            "{}/{} must seed at the visibility the native pane defaulted to \
             ({}): the handover changes which code draws the pane, not whether the \
             pane is on screen",
            handed.plugin,
            handed.pane,
            if handed.native_default_visible {
                "visible"
            } else {
                "hidden"
            }
        );
        assert_eq!(
            pane.toggle_action.is_some(),
            !handed.native_default_visible,
            "{}/{}: a pane that seeds hidden must bind the action that showed the \
             native one, and a pane that seeds visible must bind none — the native \
             pane it replaced had no toggle",
            handed.plugin,
            handed.pane
        );
    }
}

/// No bundled reproduction declares one of thurbox's own keyboards (ADR-51), for the
/// reason ADR-47 gave the toggle fields: while the native pane is still drawn, a
/// reproduction that inherited its keyboard would be focused as *that pane* — two
/// panes drawn as focused, and a cursor a user moves in one while looking at the
/// other.
///
/// The field is not a reproduction's to declare; it is the handover's. So a
/// handed-over pane is not merely exempted — it is **required** to declare one, since
/// a pane that is the interface's pane for a keyboard and does not say so is a pane
/// whose keys stopped resolving. Both directions come off the same list.
#[test]
fn only_a_handed_over_pane_declares_a_kernel_keyboard() {
    for (plugin, dir) in bundled_plugins() {
        for pane in manifest_at(&dir).panes {
            let handed_over = PANES_DRAWN_IN_A_NATIVE_PANES_PLACE
                .iter()
                .any(|h| h.plugin == plugin && h.pane == pane.id);
            if handed_over {
                continue;
            }
            assert!(
                pane.key_context.is_none(),
                "{plugin}/{} is a reproduction and declares the `{:?}` keyboard: while thurbox \
                 still draws that pane, both would be focused as it. Declare it in the change \
                 that stops drawing the native pane, and add the pane to \
                 PANES_DRAWN_IN_A_NATIVE_PANES_PLACE",
                pane.id,
                pane.key_context,
            );
        }
    }
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
