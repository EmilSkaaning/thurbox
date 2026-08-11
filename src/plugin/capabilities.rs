//! Granting host powers to a plugin.
//!
//! Enforcement here is **by absence**. A plugin's module table is built from
//! the capability set its manifest declared, and a binding it did not ask for
//! is never inserted — there is no runtime permission check inside a binding
//! that a future contributor could forget to write. The table is then frozen,
//! so the surface cannot be rewritten from inside the VM.
//!
//! What a plugin *can* still do is shadow a global — including `require` — in
//! its own VM's environment, because Luau's sandbox freezes the shared globals
//! but leaves each VM a writable layer above them. This grants nothing: the
//! value it gets back is one it wrote itself, and the host's record of granted
//! capabilities is unaffected. It is confined to the one VM, which is why the
//! runtime gives every plugin its own.

use std::collections::BTreeSet;

use mlua::{Lua, Table};

use crate::session::plugin_manifest::Capability;

/// The capabilities one plugin was actually granted.
///
/// Distinct from the manifest's requested set so that a future policy layer
/// (a user denying a capability an installed plugin asked for) has somewhere to
/// live without changing every call site.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GrantedCapabilities {
    granted: BTreeSet<Capability>,
}

impl GrantedCapabilities {
    /// Grant exactly what the manifest requested.
    ///
    /// Validation already rejected unknown names at the manifest stage, so
    /// anything reaching here is a capability this build defines.
    pub fn from_manifest(requested: &BTreeSet<Capability>) -> Self {
        Self {
            granted: requested.clone(),
        }
    }

    /// Grant nothing.
    pub fn none() -> Self {
        Self::default()
    }

    /// Whether a capability was granted.
    pub fn has(&self, capability: Capability) -> bool {
        self.granted.contains(&capability)
    }

    /// The granted set, for reporting alongside a plugin's status.
    pub fn iter(&self) -> impl Iterator<Item = Capability> + '_ {
        self.granted.iter().copied()
    }

    /// Whether nothing was granted.
    pub fn is_empty(&self) -> bool {
        self.granted.is_empty()
    }
}

/// Build the `@thurbox` module table for one plugin.
///
/// Every binding is inserted under the capability that guards it; a capability
/// that was not granted contributes no key at all, so a plugin cannot even
/// discover the shape of what it lacks.
///
/// The bindings themselves are deliberately thin — this change introduces the
/// enforcement mechanism, and each real host power arrives with the change that
/// needs it.
pub fn build_module_table(
    lua: &Lua,
    plugin_name: &str,
    granted: &GrantedCapabilities,
    store: Option<Box<dyn crate::session::plugin_store::PluginStore>>,
) -> mlua::Result<Table> {
    let module = lua.create_table()?;

    // Identity is not a capability: a plugin always knows which plugin it is,
    // and withholding that would only make error messages worse.
    module.set("name", plugin_name.to_string())?;

    if granted.has(Capability::Log) {
        let name = plugin_name.to_string();
        let log = lua.create_function(move |_, message: String| {
            tracing::info!(plugin = %name, "{message}");
            Ok(())
        })?;
        module.set("log", log)?;
    }

    // Durable storage. `Rc` because both bindings share one store and a VM is
    // single-threaded by construction; the plugin name is baked in here so a
    // plugin never names its own namespace.
    let store = store.map(std::rc::Rc::new);

    if granted.has(Capability::StateRead) {
        let name = plugin_name.to_string();
        let reader = store.clone();
        let read = lua.create_function(move |_, key: String| {
            let Some(store) = reader.as_ref() else {
                return Err(mlua::Error::runtime("plugin storage is unavailable"));
            };
            store.get(&name, &key).map_err(mlua::Error::runtime)
        })?;
        module.set("stateRead", read)?;
    }

    if granted.has(Capability::StateWrite) {
        let name = plugin_name.to_string();
        let writer = store.clone();
        let write = lua.create_function(move |_, (key, value): (String, String)| {
            let Some(store) = writer.as_ref() else {
                return Err(mlua::Error::runtime("plugin storage is unavailable"));
            };
            store.set(&name, &key, &value).map_err(mlua::Error::runtime)
        })?;
        module.set("stateWrite", write)?;

        let name = plugin_name.to_string();
        let deleter = store.clone();
        let delete = lua.create_function(move |_, key: String| {
            let Some(store) = deleter.as_ref() else {
                return Err(mlua::Error::runtime("plugin storage is unavailable"));
            };
            store.delete(&name, &key).map_err(mlua::Error::runtime)
        })?;
        module.set("stateDelete", delete)?;
    }

    // Kernel-state readers. Each reads the snapshot published by
    // `session::pane_context` at the moment it is called, so a plugin sees the
    // most recent publication without the host pushing anything into its VM —
    // and nothing here can reach the running application, only a frozen value.
    //
    // Gated per *kind* of state rather than by one blanket grant, because the
    // capability list is what an install prompt is written from.
    if granted.has(Capability::Sessions) {
        let read = lua.create_function(|lua, ()| {
            let Some(context) = crate::session::pane_context::published() else {
                // Nothing published yet: a plugin that renders before the first
                // tick sees no session, which is the same answer it gets when
                // none is open. Both are states it must handle anyway.
                return Ok(mlua::Value::Nil);
            };
            super::kernel_state::session_table(lua, &context)
        })?;
        module.set("activeSession", read)?;

        // The whole rendered list, under the *same* grant. The capability's
        // sentence is already "read the sessions thurbox is running" — plural —
        // and both readers answer the one question a user is being asked.
        // Splitting them would put two questions in an install prompt for one
        // disclosure, and would make a pane that draws the session list demand
        // two grants to draw one pane.
        let read_list = lua.create_function(|lua, ()| {
            let context = crate::session::pane_context::published().unwrap_or_default();
            super::kernel_state::session_list_table(lua, &context)
        })?;
        module.set("sessionList", read_list)?;
    }

    if granted.has(Capability::Metrics) {
        let read = lua.create_function(|lua, ()| {
            let Some(context) = crate::session::pane_context::published() else {
                return Ok(mlua::Value::Nil);
            };
            super::kernel_state::metrics_table(lua, &context)
        })?;
        module.set("systemMetrics", read)?;
    }

    if granted.has(Capability::Automations) {
        let read = lua.create_function(|lua, ()| {
            let context = crate::session::pane_context::published().unwrap_or_default();
            super::kernel_state::automations_table(lua, &context)
        })?;
        module.set("upcomingAutomations", read)?;
    }

    if granted.has(Capability::Tasks) {
        let read = lua.create_function(|lua, ()| {
            let context = crate::session::pane_context::published().unwrap_or_default();
            super::kernel_state::tasks_table(lua, &context)
        })?;
        module.set("tasks", read)?;
    }

    // Reads the file tree the file viewer has open — and nothing else. There is
    // deliberately no companion binding that lists a directory or reads a file:
    // the pane's rows are kernel view state (which directories the user
    // expanded, where the cursor is, what the search matched), so a filesystem
    // binding would be strictly more power for strictly less result. Denial
    // stays by absence, so the way to keep it that way is not to write one.
    if granted.has(Capability::Files) {
        let read = lua.create_function(|lua, ()| {
            let context = crate::session::pane_context::published().unwrap_or_default();
            super::kernel_state::files_table(lua, &context)
        })?;
        module.set("files", read)?;
    }

    // Reads the diff the code-review view has open — and nothing else. There is
    // deliberately no companion binding that produces a diff, names a revision
    // range, or reads a file: the rows are the review the *user* opened, so a
    // git binding would be strictly more power for strictly less result. Same
    // shape, and the same reasoning, as `files` above.
    if granted.has(Capability::Review) {
        let read = lua.create_function(|lua, ()| {
            let context = crate::session::pane_context::published().unwrap_or_default();
            super::kernel_state::review_table(lua, &context)
        })?;
        module.set("review", read)?;
    }

    // View-node constructors. Ungated on purpose: they build plain tables and
    // grant no host power, so hiding them behind a capability would be theatre.
    // Implemented here rather than as a shipped `.luau` file so they live in
    // the frozen module table — a plugin cannot replace `ui.text` for itself
    // and then be surprised by what the host converts.
    module.set("ui", build_ui_table(lua)?)?;

    // Frozen so the capability surface cannot be rewritten from inside the VM.
    // Without this a plugin could assign its own function to a binding name it
    // was not granted — which grants it no host power, but makes the table a
    // liar about what the plugin holds, and any later host code that reads it
    // back would be reading plugin-controlled values.
    module.set_readonly(true);

    Ok(module)
}

/// Write a constructor's `style` argument onto the node it belongs to.
///
/// Two accepted spellings, one node: a **string** is the style token (the form
/// every plugin already written uses), and a **table** names the token under
/// `token` plus any of the emphases, the selection role and the tint. Both land
/// as the same flat node fields, so conversion learns nothing about which form
/// was used and a pane's appearance cannot depend on how it was spelled.
///
/// Anything else is left alone rather than rejected here: conversion owns every
/// field's type check, and validating in two places is how the two come to
/// disagree about what a style is.
fn apply_style_arg(node: &Table, style: Option<mlua::Value>) -> mlua::Result<()> {
    match style {
        Some(mlua::Value::Table(t)) => {
            if let Ok(token) = t.get::<mlua::Value>("token") {
                if !token.is_nil() {
                    node.set("style", token)?;
                }
            }
            for key in ["bold", "dim", "underline", "selected", "tint"] {
                if let Ok(value) = t.get::<mlua::Value>(key) {
                    if !value.is_nil() {
                        node.set(key, value)?;
                    }
                }
            }
        }
        Some(value) if !value.is_nil() => node.set("style", value)?,
        _ => {}
    }
    Ok(())
}

/// Build the `ui` constructor table.
///
/// One constructor per node kind, so a plugin never writes a `kind` string it
/// can misspell into a runtime rejection.
fn build_ui_table(lua: &Lua) -> mlua::Result<Table> {
    let ui = lua.create_table()?;

    // `ui.text(content, style?, bold?, underline?, dim?, selected?)`
    //
    // The flags are positional rather than an options table because the node's
    // own fields are the canonical form and `bold` cannot stop being the third
    // argument without breaking every plugin already written against it — two
    // spellings of one node would be worse than one long signature. Six was the
    // practical limit of that form, and the note left here said a seventh field
    // should convert the flags to a table rather than continue. `tint` is that
    // seventh, so `style` now also accepts a **table** naming every field:
    // `ui.text(t, { token = "muted", tint = "added" })`. The positional form is
    // untouched, argument for argument, so nothing written against it moved.
    type TextArgs = (
        mlua::Value,
        Option<mlua::Value>,
        Option<bool>,
        Option<bool>,
        Option<bool>,
        Option<bool>,
    );
    ui.set(
        "text",
        lua.create_function(
            |lua, (content, style, bold, underline, dim, selected): TextArgs| {
                let node = lua.create_table()?;
                node.set("kind", "text")?;
                node.set("content", content)?;
                apply_style_arg(&node, style)?;
                // Only a `true` is carried, so a node table stays as small as
                // what it actually declares.
                for (key, flag) in [
                    ("bold", bold),
                    ("underline", underline),
                    ("dim", dim),
                    ("selected", selected),
                ] {
                    if let Some(true) = flag {
                        node.set(key, true)?;
                    }
                }
                Ok(node)
            },
        )?,
    )?;

    // `ui.fill(glyph, style?)` — one glyph repeated across whatever width is
    // left on the line. Only two arguments, so the style table is the only form
    // that reaches every field; there is deliberately no positional flag list
    // here to keep in step with `text`'s.
    ui.set(
        "fill",
        lua.create_function(|lua, (glyph, style): (mlua::Value, Option<mlua::Value>)| {
            let node = lua.create_table()?;
            node.set("kind", "fill")?;
            node.set("glyph", glyph)?;
            apply_style_arg(&node, style)?;
            Ok(node)
        })?,
    )?;

    // `line` shares the container shape but not the layout: its children are
    // packed at their own width on one row, which is what a `label: value` row
    // needs and what `row`'s equal shares cannot express.
    // `paragraph` shares `line`'s inline children but wraps instead of clipping,
    // so an unbounded value stays readable where a fixed-width row must not push
    // its neighbours down.
    for kind in ["row", "line", "paragraph", "column"] {
        ui.set(
            kind,
            lua.create_function(move |lua, children: Option<Table>| {
                let node = lua.create_table()?;
                node.set("kind", kind)?;
                node.set("children", children.unwrap_or(lua.create_table()?))?;
                Ok(node)
            })?,
        )?;
    }

    // `ui.list(children, selected?)` — the one container that may name the row
    // its cursor is on, so the kernel can scroll it to that row from a height
    // the plugin is never told. One-based, like the table it indexes.
    ui.set(
        "list",
        lua.create_function(
            |lua, (children, selected_row): (Option<Table>, Option<mlua::Value>)| {
                let node = lua.create_table()?;
                node.set("kind", "list")?;
                node.set("children", children.unwrap_or(lua.create_table()?))?;
                // Passed through rather than validated here: conversion owns the
                // range check, so a bad index is one named error rather than two
                // that can disagree.
                if let Some(row) = selected_row.filter(|v| !v.is_nil()) {
                    node.set("selectedRow", row)?;
                }
                Ok(node)
            },
        )?,
    )?;

    ui.set(
        "divider",
        lua.create_function(|lua, ()| {
            let node = lua.create_table()?;
            node.set("kind", "divider")?;
            Ok(node)
        })?,
    )?;

    // `ui.cycle(id, frames, fps?, loop?)` — declared motion (ADR-V18).
    //
    // A constructor rather than a `motion` field on every other constructor:
    // the frames *are* the node's content, so a cycle has nothing else to
    // carry, and one function keeps a plugin from spelling the declaration by
    // hand. The kernel drives it; there is deliberately no call by which a
    // plugin advances or requests a frame.
    ui.set(
        "cycle",
        lua.create_function(
            |lua, (id, frames, fps, repeat_): (String, Table, Option<u16>, Option<bool>)| {
                let motion = lua.create_table()?;
                motion.set("kind", "cycle")?;
                motion.set("frames", frames)?;
                if let Some(fps) = fps {
                    motion.set("fps", fps)?;
                }
                if let Some(false) = repeat_ {
                    motion.set("loop", false)?;
                }
                let node = lua.create_table()?;
                node.set("kind", "motion")?;
                node.set("id", id)?;
                node.set("motion", motion)?;
                Ok(node)
            },
        )?,
    )?;

    // `ui.gauge(label, percent, suffix?)` — a labelled bar whose geometry the
    // kernel resolves. It exists because a plugin never learns its pane's width,
    // so it could not right-align a suffix or size a bar itself.
    ui.set(
        "gauge",
        lua.create_function(
            |lua, (label, percent, suffix): (String, f64, Option<String>)| {
                let node = lua.create_table()?;
                node.set("kind", "gauge")?;
                node.set("label", label)?;
                node.set("percent", percent)?;
                if let Some(suffix) = suffix {
                    node.set("suffix", suffix)?;
                }
                Ok(node)
            },
        )?,
    )?;

    ui.set(
        "spacer",
        lua.create_function(|lua, lines: Option<u16>| {
            let node = lua.create_table()?;
            node.set("kind", "spacer")?;
            node.set("lines", lines.unwrap_or(1))?;
            Ok(node)
        })?,
    )?;

    ui.set_readonly(true);
    Ok(ui)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn set(caps: &[Capability]) -> BTreeSet<Capability> {
        caps.iter().copied().collect()
    }

    #[test]
    fn empty_declaration_grants_nothing() {
        let granted = GrantedCapabilities::from_manifest(&BTreeSet::new());
        assert!(granted.is_empty());
        for c in Capability::all() {
            assert!(!granted.has(*c));
        }
    }

    #[test]
    fn granted_set_is_reportable() {
        let granted =
            GrantedCapabilities::from_manifest(&set(&[Capability::Log, Capability::StateRead]));
        let reported: Vec<Capability> = granted.iter().collect();
        assert_eq!(reported, vec![Capability::Log, Capability::StateRead]);
    }

    #[test]
    fn unknown_capability_never_reaches_the_grant_stage() {
        // The vocabulary is closed at the manifest layer, so a grant set can
        // only ever be built from names this build defines.
        assert!(Capability::from_str("read-everything").is_err());
    }

    #[test]
    fn declared_binding_is_present() {
        let lua = Lua::new();
        lua.set_named_registry_value("thurbox_plugin_state", lua.create_table().unwrap())
            .unwrap();
        let granted = GrantedCapabilities::from_manifest(&set(&[Capability::Log]));
        let module = build_module_table(&lua, "demo", &granted, None).unwrap();

        assert!(module.contains_key("log").unwrap());
        assert!(module.get::<mlua::Function>("log").is_ok());
    }

    #[test]
    fn undeclared_binding_is_absent_not_refusing() {
        let lua = Lua::new();
        let granted = GrantedCapabilities::none();
        let module = build_module_table(&lua, "demo", &granted, None).unwrap();

        assert!(!module.contains_key("log").unwrap());
        assert!(!module.contains_key("stateRead").unwrap());
        assert!(!module.contains_key("stateWrite").unwrap());
        // Absent, not a function that errors when called.
        assert!(matches!(
            module.get::<mlua::Value>("log").unwrap(),
            mlua::Value::Nil
        ));
    }

    #[test]
    fn two_plugins_get_different_tables() {
        let lua = Lua::new();
        lua.set_named_registry_value("thurbox_plugin_state", lua.create_table().unwrap())
            .unwrap();

        let with = build_module_table(
            &lua,
            "with",
            &GrantedCapabilities::from_manifest(&set(&[Capability::Log])),
            None,
        )
        .unwrap();
        let without =
            build_module_table(&lua, "without", &GrantedCapabilities::none(), None).unwrap();

        assert!(with.contains_key("log").unwrap());
        assert!(!without.contains_key("log").unwrap());
    }

    #[test]
    fn read_and_write_are_separate_capabilities() {
        let lua = Lua::new();
        lua.set_named_registry_value("thurbox_plugin_state", lua.create_table().unwrap())
            .unwrap();
        let read_only = build_module_table(
            &lua,
            "reader",
            &GrantedCapabilities::from_manifest(&set(&[Capability::StateRead])),
            None,
        )
        .unwrap();

        assert!(read_only.contains_key("stateRead").unwrap());
        assert!(!read_only.contains_key("stateWrite").unwrap());
    }

    #[test]
    fn the_module_table_is_frozen() {
        let lua = Lua::new();
        let module = build_module_table(&lua, "demo", &GrantedCapabilities::none(), None).unwrap();
        assert!(module.is_readonly());
        assert!(
            module.set("log", true).is_err(),
            "a plugin must not be able to add a binding name it was denied"
        );
    }

    #[test]
    fn ui_constructors_are_present_without_any_capability() {
        let lua = Lua::new();
        let module = build_module_table(&lua, "demo", &GrantedCapabilities::none(), None).unwrap();
        let ui: Table = module.get("ui").expect("ui table present");
        for name in [
            "text",
            "row",
            "line",
            "paragraph",
            "column",
            "list",
            "divider",
            "fill",
            "gauge",
            "spacer",
            "cycle",
        ] {
            assert!(ui.contains_key(name).unwrap(), "missing ui.{name}");
        }
    }

    #[test]
    fn the_ui_table_is_frozen_too() {
        let lua = Lua::new();
        let module = build_module_table(&lua, "demo", &GrantedCapabilities::none(), None).unwrap();
        let ui: Table = module.get("ui").unwrap();
        assert!(ui.is_readonly());
    }

    /// Kernel state is gated per kind, so one grant must not imply another:
    /// a pane that wants a session name must not thereby read host telemetry.
    #[test]
    fn each_state_reader_is_gated_by_its_own_capability() {
        let lua = Lua::new();
        for (capability, present, absent) in [
            (
                Capability::Sessions,
                "activeSession",
                ["systemMetrics", "upcomingAutomations", "tasks", "files"],
            ),
            (
                Capability::Metrics,
                "systemMetrics",
                ["activeSession", "upcomingAutomations", "tasks", "files"],
            ),
            (
                Capability::Automations,
                "upcomingAutomations",
                ["activeSession", "systemMetrics", "tasks", "files"],
            ),
            (
                Capability::Tasks,
                "tasks",
                [
                    "activeSession",
                    "systemMetrics",
                    "upcomingAutomations",
                    "files",
                ],
            ),
            (
                Capability::Files,
                "files",
                [
                    "activeSession",
                    "systemMetrics",
                    "upcomingAutomations",
                    "tasks",
                ],
            ),
            (
                Capability::Review,
                "review",
                [
                    "activeSession",
                    "systemMetrics",
                    "upcomingAutomations",
                    "tasks",
                ],
            ),
        ] {
            let module = build_module_table(
                &lua,
                "demo",
                &GrantedCapabilities::from_manifest(&set(&[capability])),
                None,
            )
            .unwrap();
            assert!(
                module.contains_key(present).unwrap(),
                "{capability} should grant {present}"
            );
            for name in absent {
                assert!(
                    !module.contains_key(name).unwrap(),
                    "{capability} must not grant {name}"
                );
            }
        }
    }

    #[test]
    fn no_state_capability_means_no_state_reader() {
        let lua = Lua::new();
        let module = build_module_table(&lua, "demo", &GrantedCapabilities::none(), None).unwrap();
        for name in [
            "activeSession",
            "systemMetrics",
            "upcomingAutomations",
            "tasks",
            "files",
            "review",
        ] {
            assert!(!module.contains_key(name).unwrap(), "{name} leaked");
        }
    }

    /// Before the first publication the readers answer "nothing", rather than
    /// erroring — a plugin rendering on its first worker cycle is normal.
    #[test]
    fn a_reader_called_before_anything_is_published_answers_nothing() {
        let _guard = crate::session::pane_context::test_lock();
        crate::session::pane_context::clear_for_test();
        let lua = Lua::new();
        let module = build_module_table(
            &lua,
            "demo",
            &GrantedCapabilities::from_manifest(&set(&[
                Capability::Sessions,
                Capability::Metrics,
                Capability::Automations,
                Capability::Tasks,
                Capability::Files,
                Capability::Review,
            ])),
            None,
        )
        .unwrap();
        let session: mlua::Value = module
            .get::<mlua::Function>("activeSession")
            .unwrap()
            .call(())
            .unwrap();
        assert!(matches!(session, mlua::Value::Nil));
        let automations: Table = module
            .get::<mlua::Function>("upcomingAutomations")
            .unwrap()
            .call(())
            .unwrap();
        assert_eq!(automations.raw_len(), 0);
        let tasks: Table = module
            .get::<mlua::Function>("tasks")
            .unwrap()
            .call(())
            .unwrap();
        assert_eq!(tasks.get::<Table>("entries").unwrap().raw_len(), 0);
        let files: Table = module
            .get::<mlua::Function>("files")
            .unwrap()
            .call(())
            .unwrap();
        assert_eq!(files.get::<Table>("nodes").unwrap().raw_len(), 0);
        assert!(matches!(
            files.get::<mlua::Value>("selected").unwrap(),
            mlua::Value::Nil
        ));
        let review: Table = module
            .get::<mlua::Function>("review")
            .unwrap()
            .call(())
            .unwrap();
        assert_eq!(review.get::<Table>("lines").unwrap().raw_len(), 0);
    }

    /// The `review` capability sounds like access to a repository, so what it does
    /// not insert is asserted directly: nothing a plugin holding it can call runs
    /// git, produces a diff, or reads a file.
    #[test]
    fn the_review_capability_inserts_no_repository_binding() {
        let lua = Lua::new();
        let module = build_module_table(
            &lua,
            "demo",
            &GrantedCapabilities::from_manifest(&set(&[Capability::Review])),
            None,
        )
        .unwrap();
        for name in [
            "git", "diff", "commits", "log", "checkout", "readFile", "exec",
        ] {
            assert!(
                !module.contains_key(name).unwrap(),
                "granting `review` must not insert `{name}`"
            );
        }
        assert!(
            Capability::from_str("git").is_err(),
            "the vocabulary must define no version-control capability"
        );
    }

    /// The `files` capability is the widest-*sounding* grant in the host, so what
    /// it does not insert is asserted directly: there is no binding through which
    /// a plugin holding it can reach the filesystem.
    #[test]
    fn the_file_capability_inserts_no_filesystem_binding() {
        let lua = Lua::new();
        let module = build_module_table(
            &lua,
            "demo",
            &GrantedCapabilities::from_manifest(&set(&[Capability::Files])),
            None,
        )
        .unwrap();
        for name in [
            "readDir",
            "readFile",
            "listDir",
            "stat",
            "openFile",
            "writeFile",
            "fs",
            "path",
        ] {
            assert!(
                !module.contains_key(name).unwrap(),
                "granting `files` must not insert `{name}`"
            );
        }
        // And the host defines no filesystem capability at all, so none could be
        // granted even by a manifest that asked.
        assert!(
            Capability::from_str("fs").is_err(),
            "the vocabulary must define no filesystem capability"
        );
    }

    #[test]
    fn plugin_always_knows_its_own_name() {
        let lua = Lua::new();
        let module = build_module_table(&lua, "demo", &GrantedCapabilities::none(), None).unwrap();
        assert_eq!(module.get::<String>("name").unwrap(), "demo");
    }
}
