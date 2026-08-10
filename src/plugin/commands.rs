//! Turning discovered manifests into the command registry, and a command's
//! Lua return value into JSON.
//!
//! Like [`super::spawn`], the registry side involves no VM: a command's id,
//! title and argument schema are manifest facts, so listing them costs a
//! filesystem walk rather than a VM per plugin. That is what lets
//! `thurbox-cli command list` answer for a plugin that has never started or
//! whose code faults — the failure modes that force `plugin list` to start
//! plugins do not apply to a question the manifest already answers.
//!
//! The conversion side does run on the plugin's thread, inside its interrupt and
//! memory bounds. It is still bounded separately, because the value it produces
//! is dropped on the *host* side: a deeply nested table would otherwise become a
//! deeply nested [`serde_json::Value`] whose recursive drop is the host's stack,
//! not the VM's.

use mlua::Value as LuaValue;
use serde_json::{Map, Value};

use crate::session::plugin_command::CommandRegistry;

use super::discovery::DiscoveredPlugin;

/// Deepest table nesting a command may return.
///
/// The same figure the view tree uses: a value a human reads or an agent parses
/// has no business nesting further, and the limit is what keeps the host's drop
/// off a recursion cliff.
pub const MAX_RESULT_DEPTH: usize = 32;

/// Most values a command's result may hold, counted across the whole structure.
///
/// Breadth needs its own bound: a flat table of a million entries passes any
/// depth check.
pub const MAX_RESULT_NODES: usize = 4096;

/// Build the command registry for a set of discovered plugins.
pub fn registry_for<'a>(
    plugins: impl IntoIterator<Item = &'a DiscoveredPlugin>,
) -> CommandRegistry {
    CommandRegistry::from_manifests(plugins.into_iter().map(|p| &p.manifest))
}

/// Why a command's return value could not be represented.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultError {
    /// Nested past [`MAX_RESULT_DEPTH`].
    TooDeep,
    /// Larger than [`MAX_RESULT_NODES`] values.
    TooLarge,
    /// A Lua type with no JSON counterpart.
    Unrepresentable {
        /// The offending type's name.
        got: String,
    },
}

impl std::fmt::Display for ResultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResultError::TooDeep => {
                write!(
                    f,
                    "command result nests deeper than {MAX_RESULT_DEPTH} levels"
                )
            }
            ResultError::TooLarge => {
                write!(
                    f,
                    "command result holds more than {MAX_RESULT_NODES} values"
                )
            }
            ResultError::Unrepresentable { got } => {
                write!(f, "command result holds a `{got}`, which is not JSON")
            }
        }
    }
}

impl std::error::Error for ResultError {}

/// Convert a command's return value to JSON.
///
/// Lua's single table type is ambiguous, so the rule is stated rather than
/// guessed: a table holding key `1` converts as an **array** over its dense
/// prefix, anything else as an **object** with stringified keys, and an empty
/// table as an object. An unrepresentable value is an error naming its type,
/// not a silent `null` — a plugin returning a closure has a bug, and hiding it
/// costs the author the error message.
pub fn to_json(value: &LuaValue) -> Result<Value, ResultError> {
    let mut budget = MAX_RESULT_NODES;
    convert(value, 1, &mut budget)
}

/// Walk one value.
fn convert(value: &LuaValue, depth: usize, budget: &mut usize) -> Result<Value, ResultError> {
    // Checked before touching a table, so a self-referencing table stops here
    // rather than recursing until the stack gives out.
    if depth > MAX_RESULT_DEPTH {
        return Err(ResultError::TooDeep);
    }
    if *budget == 0 {
        return Err(ResultError::TooLarge);
    }
    *budget -= 1;

    match value {
        LuaValue::Nil => Ok(Value::Null),
        LuaValue::Boolean(b) => Ok(Value::Bool(*b)),
        LuaValue::Integer(i) => Ok(Value::from(*i)),
        LuaValue::Number(n) => Ok(serde_json::Number::from_f64(*n)
            .map(Value::Number)
            // A non-finite number has no JSON form; naming it is more useful
            // than emitting `null` and letting the caller wonder.
            .ok_or_else(|| ResultError::Unrepresentable {
                got: format!("number {n}"),
            })?),
        LuaValue::String(s) => Ok(Value::String(s.to_string_lossy().to_string())),
        LuaValue::Table(table) => convert_table(table, depth, budget),
        other => Err(ResultError::Unrepresentable {
            got: other.type_name().to_string(),
        }),
    }
}

/// Convert a table as an array or an object.
fn convert_table(
    table: &mlua::Table,
    depth: usize,
    budget: &mut usize,
) -> Result<Value, ResultError> {
    let sequence = matches!(table.get::<LuaValue>(1), Ok(v) if v != LuaValue::Nil);
    if sequence {
        let mut items = Vec::new();
        // The dense prefix only: a table mixing `1..n` with string keys is a
        // sequence here, and the strays are dropped rather than making the
        // result's shape depend on iteration order.
        for index in 1.. {
            match table.get::<LuaValue>(index) {
                Ok(LuaValue::Nil) | Err(_) => break,
                Ok(item) => items.push(convert(&item, depth + 1, budget)?),
            }
        }
        return Ok(Value::Array(items));
    }

    let mut object = Map::new();
    for pair in table.pairs::<LuaValue, LuaValue>() {
        let (key, value) = pair.map_err(|e| ResultError::Unrepresentable { got: e.to_string() })?;
        let key = match &key {
            LuaValue::String(s) => s.to_string_lossy().to_string(),
            LuaValue::Integer(i) => i.to_string(),
            LuaValue::Number(n) => n.to_string(),
            other => {
                return Err(ResultError::Unrepresentable {
                    got: format!("{} key", other.type_name()),
                })
            }
        };
        object.insert(key, convert(&value, depth + 1, budget)?);
    }
    Ok(Value::Object(object))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::discovery::discover_in;
    use crate::session::plugin_command::Handler;
    use std::fs;
    use std::path::Path;

    fn write_plugin(root: &Path, name: &str, manifest_extra: &str, entry: &str) {
        let dir = root.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("plugin.toml"),
            format!("name = \"{name}\"\napi_version = 1\n{manifest_extra}"),
        )
        .unwrap();
        fs::write(dir.join("init.luau"), entry).unwrap();
    }

    fn registry_over(root: &Path) -> CommandRegistry {
        registry_for(discover_in(&[], Some(root)).loadable.iter())
    }

    #[test]
    fn a_declared_command_is_registered_without_a_vm() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "notes",
            "[[commands]]\nid = \"add\"\ntitle = \"Add\"\n",
            // Would fail loudly if it were ever executed.
            "error('must not run')",
        );

        let registry = registry_over(tmp.path());
        let spec = registry.get("notes.add").expect("registered");
        assert_eq!(spec.title, "Add");
        assert_eq!(
            spec.handler,
            Handler::Plugin {
                entry: "add".to_string()
            }
        );
    }

    #[test]
    fn a_plugin_whose_code_is_broken_still_contributes_commands() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "broken",
            "[[commands]]\nid = \"go\"\n",
            "return {",
        );

        // The registry is manifest-derived, so "does this plugin work" and
        // "what does it offer" are answered separately.
        assert!(registry_over(tmp.path()).get("broken.go").is_some());
    }

    #[test]
    fn a_pane_only_plugin_contributes_its_visibility_commands() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(
            tmp.path(),
            "board",
            "capabilities = [\"render\"]\n[[panes]]\nid = \"main\"\n",
            "return {}",
        );

        let registry = registry_over(tmp.path());
        assert_eq!(registry.len(), 3);
        assert!(registry.get("board.main.toggle").is_some());
        assert!(registry.get("board.main.show").is_some());
        assert!(registry.get("board.main.hide").is_some());
    }

    #[test]
    fn a_plugin_declaring_nothing_registers_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        write_plugin(tmp.path(), "quiet", "", "return {}");
        assert!(registry_over(tmp.path()).is_empty());
    }

    /// Evaluate a chunk and convert its result, as a command dispatch does.
    fn converted(source: &str) -> Result<Value, ResultError> {
        let lua = mlua::Lua::new();
        let value: LuaValue = lua.load(source).eval().expect("chunk evaluates");
        to_json(&value)
    }

    #[test]
    fn scalars_convert() {
        assert_eq!(converted("return nil").unwrap(), Value::Null);
        assert_eq!(converted("return true").unwrap(), Value::Bool(true));
        assert_eq!(converted("return 7").unwrap(), Value::from(7));
        assert_eq!(converted("return 'hi'").unwrap(), Value::from("hi"));
    }

    #[test]
    fn a_table_of_keys_becomes_an_object() {
        assert_eq!(
            converted("return { id = 'x', done = true }").unwrap(),
            serde_json::json!({ "id": "x", "done": true })
        );
    }

    #[test]
    fn a_sequence_becomes_an_array_in_order() {
        assert_eq!(
            converted("return { 'a', 'b', 'c' }").unwrap(),
            serde_json::json!(["a", "b", "c"])
        );
    }

    #[test]
    fn an_empty_table_is_an_object() {
        // Ambiguous in Lua, so the rule is stated: no `1` key means an object.
        assert_eq!(converted("return {}").unwrap(), serde_json::json!({}));
    }

    #[test]
    fn nesting_is_preserved() {
        assert_eq!(
            converted("return { items = { { n = 1 }, { n = 2 } } }").unwrap(),
            serde_json::json!({ "items": [{ "n": 1 }, { "n": 2 }] })
        );
    }

    #[test]
    fn an_excessively_deep_result_is_refused() {
        let source = format!(
            "local v = 1 for _ = 1, {} do v = {{ v }} end return v",
            MAX_RESULT_DEPTH + 5
        );
        assert_eq!(converted(&source), Err(ResultError::TooDeep));
    }

    #[test]
    fn a_self_referencing_table_is_refused_rather_than_looping() {
        assert_eq!(
            converted("local t = {} t.self = t return t"),
            Err(ResultError::TooDeep)
        );
    }

    #[test]
    fn an_oversized_result_is_refused() {
        let source = format!(
            "local t = {{}} for i = 1, {} do t[i] = i end return t",
            MAX_RESULT_NODES + 10
        );
        assert_eq!(converted(&source), Err(ResultError::TooLarge));
    }

    #[test]
    fn a_function_is_refused_naming_its_type() {
        let err = converted("return function() end").expect_err("not JSON");
        assert_eq!(
            err,
            ResultError::Unrepresentable {
                got: "function".to_string()
            }
        );
        assert!(err.to_string().contains("function"), "{err}");
    }

    #[test]
    fn a_non_finite_number_is_refused() {
        let err = converted("return 0/0").expect_err("nan is not JSON");
        assert!(
            matches!(err, ResultError::Unrepresentable { .. }),
            "{err:?}"
        );
    }
}
