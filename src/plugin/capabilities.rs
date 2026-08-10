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

/// Build the `ui` constructor table.
///
/// One constructor per node kind, so a plugin never writes a `kind` string it
/// can misspell into a runtime rejection.
fn build_ui_table(lua: &Lua) -> mlua::Result<Table> {
    let ui = lua.create_table()?;

    // `ui.text(content, style?, bold?)`
    ui.set(
        "text",
        lua.create_function(
            |lua, (content, style, bold): (mlua::Value, Option<String>, Option<bool>)| {
                let node = lua.create_table()?;
                node.set("kind", "text")?;
                node.set("content", content)?;
                if let Some(style) = style {
                    node.set("style", style)?;
                }
                if let Some(true) = bold {
                    node.set("bold", true)?;
                }
                Ok(node)
            },
        )?,
    )?;

    // `line` shares the container shape but not the layout: its children are
    // packed at their own width on one row, which is what a `label: value` row
    // needs and what `row`'s equal shares cannot express.
    for kind in ["row", "line", "column", "list"] {
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
            "text", "row", "line", "column", "list", "divider", "spacer", "cycle",
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

    #[test]
    fn plugin_always_knows_its_own_name() {
        let lua = Lua::new();
        let module = build_module_table(&lua, "demo", &GrantedCapabilities::none(), None).unwrap();
        assert_eq!(module.get::<String>("name").unwrap(), "demo");
    }
}
