//! Converting a plugin's Lua render result into a [`ViewNode`].
//!
//! This runs on the **plugin's own thread**, inside the VM that already has an
//! instruction budget and a memory ceiling — so a pathological structure is
//! walked under those bounds rather than after escaping them. The depth and
//! node limits here are a second line, not the only one.
//!
//! Nothing a plugin can construct may panic the host: every malformed shape —
//! wrong type, unknown kind, missing field, a table containing itself — comes
//! back as a [`ViewError`].

use std::fmt;

use mlua::{Table, Value};

use crate::session::view_tree::{
    sanitize_text, StyleToken, TextStyle, ViewNode, MAX_DEPTH, MAX_NODES,
};

/// Why a render result could not become a view tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewError {
    /// The value was not a table where a node was expected.
    NotANode {
        /// What arrived instead.
        got: String,
    },
    /// The node's `kind` is not one the host defines.
    UnknownKind(String),
    /// The node omitted a field its kind requires.
    MissingField {
        /// The node kind that wanted it.
        kind: String,
        /// The field that was absent.
        field: &'static str,
    },
    /// A field had the wrong type.
    BadField {
        /// The node kind.
        kind: String,
        /// The field name.
        field: &'static str,
        /// What was expected.
        expected: &'static str,
    },
    /// The style token is not one the host defines.
    UnknownStyle(String),
    /// The tree nests deeper than the host allows. Also how a self-referential
    /// table terminates.
    TooDeep,
    /// The tree carries more nodes than the host allows.
    TooManyNodes,
}

impl fmt::Display for ViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ViewError::NotANode { got } => {
                write!(f, "expected a view node table, got {got}")
            }
            ViewError::UnknownKind(k) => write!(f, "unknown view node kind `{k}`"),
            ViewError::MissingField { kind, field } => {
                write!(f, "view node `{kind}` is missing required field `{field}`")
            }
            ViewError::BadField {
                kind,
                field,
                expected,
            } => write!(f, "view node `{kind}` field `{field}` must be {expected}"),
            ViewError::UnknownStyle(s) => write!(f, "unknown style token `{s}`"),
            ViewError::TooDeep => {
                write!(f, "view tree nests deeper than {MAX_DEPTH} levels")
            }
            ViewError::TooManyNodes => {
                write!(f, "view tree has more than {MAX_NODES} nodes")
            }
        }
    }
}

impl std::error::Error for ViewError {}

/// Convert a plugin's render result into a view tree.
pub fn from_lua(value: &Value) -> Result<ViewNode, ViewError> {
    let mut budget = MAX_NODES;
    convert(value, 1, &mut budget)
}

/// Walk one node.
///
/// `depth` starts at 1 for the root. `budget` counts down the remaining node
/// allowance across the whole tree, so breadth is bounded as well as depth —
/// a flat table of a million children would otherwise pass a depth check.
fn convert(value: &Value, depth: usize, budget: &mut usize) -> Result<ViewNode, ViewError> {
    // Checked before touching the table: a cycle reaches this and stops,
    // rather than recursing until the stack gives out.
    if depth > MAX_DEPTH {
        return Err(ViewError::TooDeep);
    }
    if *budget == 0 {
        return Err(ViewError::TooManyNodes);
    }
    *budget -= 1;

    let table = match value {
        Value::Table(t) => t,
        other => {
            return Err(ViewError::NotANode {
                got: other.type_name().to_string(),
            })
        }
    };

    let kind: String = match table.get::<Value>("kind") {
        Ok(Value::String(s)) => s.to_string_lossy().to_string(),
        Ok(Value::Nil) => {
            return Err(ViewError::MissingField {
                kind: "?".to_string(),
                field: "kind",
            })
        }
        Ok(_) => {
            return Err(ViewError::BadField {
                kind: "?".to_string(),
                field: "kind",
                expected: "a string",
            })
        }
        Err(_) => {
            return Err(ViewError::MissingField {
                kind: "?".to_string(),
                field: "kind",
            })
        }
    };

    match kind.as_str() {
        "text" => convert_text(table, &kind),
        "row" => Ok(ViewNode::Row(convert_children(
            table, &kind, depth, budget,
        )?)),
        "column" => Ok(ViewNode::Column(convert_children(
            table, &kind, depth, budget,
        )?)),
        "list" => Ok(ViewNode::List(convert_children(
            table, &kind, depth, budget,
        )?)),
        "divider" => Ok(ViewNode::Divider),
        "spacer" => {
            let lines = match table.get::<Value>("lines") {
                Ok(Value::Integer(n)) => n.clamp(0, u16::MAX as i64) as u16,
                Ok(Value::Nil) | Err(_) => 1,
                Ok(Value::Number(n)) => n.clamp(0.0, u16::MAX as f64) as u16,
                Ok(_) => {
                    return Err(ViewError::BadField {
                        kind,
                        field: "lines",
                        expected: "a number",
                    })
                }
            };
            Ok(ViewNode::Spacer { lines })
        }
        _ => Err(ViewError::UnknownKind(kind)),
    }
}

/// Convert a `text` node.
fn convert_text(table: &Table, kind: &str) -> Result<ViewNode, ViewError> {
    let content = match table.get::<Value>("content") {
        Ok(Value::String(s)) => s.to_string_lossy().to_string(),
        // Numbers are convenient and unambiguous to render; anything else is
        // a mistake worth naming rather than stringifying silently.
        Ok(Value::Integer(n)) => n.to_string(),
        Ok(Value::Number(n)) => n.to_string(),
        Ok(Value::Nil) | Err(_) => {
            return Err(ViewError::MissingField {
                kind: kind.to_string(),
                field: "content",
            })
        }
        Ok(_) => {
            return Err(ViewError::BadField {
                kind: kind.to_string(),
                field: "content",
                expected: "a string or number",
            })
        }
    };

    let token = match table.get::<Value>("style") {
        Ok(Value::String(s)) => {
            let name = s.to_string_lossy().to_string();
            Some(StyleToken::parse(&name).ok_or(ViewError::UnknownStyle(name))?)
        }
        Ok(Value::Nil) | Err(_) => None,
        Ok(_) => {
            return Err(ViewError::BadField {
                kind: kind.to_string(),
                field: "style",
                expected: "a string",
            })
        }
    };

    let bold = matches!(table.get::<Value>("bold"), Ok(Value::Boolean(true)));

    Ok(ViewNode::Text {
        content: sanitize_text(&content),
        style: TextStyle { token, bold },
    })
}

/// Convert a container's `children` array.
fn convert_children(
    table: &Table,
    kind: &str,
    depth: usize,
    budget: &mut usize,
) -> Result<Vec<ViewNode>, ViewError> {
    let children = match table.get::<Value>("children") {
        Ok(Value::Table(t)) => t,
        // A container with no children is legal and renders as empty space.
        Ok(Value::Nil) | Err(_) => return Ok(Vec::new()),
        Ok(_) => {
            return Err(ViewError::BadField {
                kind: kind.to_string(),
                field: "children",
                expected: "an array",
            })
        }
    };

    let mut out = Vec::new();
    for pair in children.sequence_values::<Value>() {
        let value = pair.map_err(|_| ViewError::BadField {
            kind: kind.to_string(),
            field: "children",
            expected: "an array",
        })?;
        out.push(convert(&value, depth + 1, budget)?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::Lua;

    /// Evaluate a Luau expression and convert its result.
    fn convert_src(src: &str) -> Result<ViewNode, ViewError> {
        let lua = Lua::new();
        let value: Value = lua.load(src).eval().expect("chunk evaluates");
        from_lua(&value)
    }

    #[test]
    fn a_text_node_converts() {
        let node = convert_src(r#"return { kind = "text", content = "hello" }"#).unwrap();
        assert_eq!(node, ViewNode::text("hello"));
    }

    #[test]
    fn a_styled_text_node_converts() {
        let node = convert_src(
            r#"return { kind = "text", content = "hi", style = "accent", bold = true }"#,
        )
        .unwrap();
        assert_eq!(
            node,
            ViewNode::styled(
                "hi",
                TextStyle {
                    token: Some(StyleToken::Accent),
                    bold: true
                }
            )
        );
    }

    #[test]
    fn nesting_is_preserved() {
        let node = convert_src(
            r#"return {
                kind = "column",
                children = {
                    { kind = "row", children = {
                        { kind = "text", content = "a" },
                        { kind = "text", content = "b" },
                    }},
                    { kind = "divider" },
                },
            }"#,
        )
        .unwrap();

        match &node {
            ViewNode::Column(children) => {
                assert_eq!(children.len(), 2);
                assert_eq!(children[0].children().len(), 2);
                assert_eq!(children[1], ViewNode::Divider);
            }
            other => panic!("expected a column, got {other:?}"),
        }
    }

    #[test]
    fn an_empty_container_is_valid() {
        assert_eq!(
            convert_src(r#"return { kind = "column" }"#).unwrap(),
            ViewNode::Column(vec![])
        );
        assert_eq!(
            convert_src(r#"return { kind = "list", children = {} }"#).unwrap(),
            ViewNode::List(vec![])
        );
    }

    #[test]
    fn a_spacer_defaults_to_one_line() {
        assert_eq!(
            convert_src(r#"return { kind = "spacer" }"#).unwrap(),
            ViewNode::Spacer { lines: 1 }
        );
        assert_eq!(
            convert_src(r#"return { kind = "spacer", lines = 3 }"#).unwrap(),
            ViewNode::Spacer { lines: 3 }
        );
    }

    #[test]
    fn a_non_table_result_is_rejected() {
        // Luau distinguishes integers from floats, so the reported type name
        // is whichever the value actually was.
        let err = convert_src("return 42").unwrap_err();
        assert_eq!(
            err,
            ViewError::NotANode {
                got: "integer".to_string()
            }
        );
        assert!(err.to_string().contains("integer"));
    }

    #[test]
    fn an_unknown_kind_is_rejected() {
        let err = convert_src(r#"return { kind = "canvas" }"#).unwrap_err();
        assert_eq!(err, ViewError::UnknownKind("canvas".to_string()));
        assert!(err.to_string().contains("canvas"));
    }

    #[test]
    fn a_missing_kind_is_rejected() {
        let err = convert_src(r#"return { content = "orphan" }"#).unwrap_err();
        assert!(matches!(err, ViewError::MissingField { field: "kind", .. }));
    }

    #[test]
    fn text_without_content_is_rejected() {
        let err = convert_src(r#"return { kind = "text" }"#).unwrap_err();
        assert_eq!(
            err,
            ViewError::MissingField {
                kind: "text".to_string(),
                field: "content"
            }
        );
    }

    #[test]
    fn an_unknown_style_token_is_rejected() {
        let err =
            convert_src(r#"return { kind = "text", content = "x", style = "neon" }"#).unwrap_err();
        assert_eq!(err, ViewError::UnknownStyle("neon".to_string()));
    }

    #[test]
    fn a_self_referential_table_terminates_via_the_depth_bound() {
        // Without the depth check this recurses until the stack dies.
        let err = convert_src(
            r#"
            local node = { kind = "column", children = {} }
            node.children[1] = node
            return node
            "#,
        )
        .unwrap_err();
        assert_eq!(err, ViewError::TooDeep);
    }

    #[test]
    fn a_tree_deeper_than_the_limit_is_rejected() {
        let src = format!(
            r#"
            local node = {{ kind = "text", content = "leaf" }}
            for _ = 1, {} do
                node = {{ kind = "column", children = {{ node }} }}
            end
            return node
            "#,
            MAX_DEPTH + 5
        );
        assert_eq!(convert_src(&src).unwrap_err(), ViewError::TooDeep);
    }

    #[test]
    fn a_tree_within_the_depth_limit_converts() {
        let src = format!(
            r#"
            local node = {{ kind = "text", content = "leaf" }}
            for _ = 1, {} do
                node = {{ kind = "column", children = {{ node }} }}
            end
            return node
            "#,
            MAX_DEPTH - 2
        );
        let node = convert_src(&src).expect("within bounds");
        assert_eq!(node.depth(), MAX_DEPTH - 1);
    }

    #[test]
    fn a_tree_with_too_many_nodes_is_rejected() {
        // Flat and shallow: only the node budget catches this.
        let src = format!(
            r#"
            local children = {{}}
            for i = 1, {} do
                children[i] = {{ kind = "text", content = "x" }}
            end
            return {{ kind = "list", children = children }}
            "#,
            MAX_NODES + 10
        );
        assert_eq!(convert_src(&src).unwrap_err(), ViewError::TooManyNodes);
    }

    #[test]
    fn escape_sequences_do_not_survive_conversion() {
        let node = convert_src(r#"return { kind = "text", content = "\27[31mred" }"#).unwrap();
        match node {
            ViewNode::Text { content, .. } => assert!(!content.contains('\x1b'), "{content:?}"),
            other => panic!("expected text, got {other:?}"),
        }
    }

    #[test]
    fn a_number_is_accepted_as_text_content() {
        let node = convert_src(r#"return { kind = "text", content = 42 }"#).unwrap();
        assert_eq!(node, ViewNode::text("42"));
    }

    #[test]
    fn a_table_as_text_content_is_rejected() {
        let err = convert_src(r#"return { kind = "text", content = {} }"#).unwrap_err();
        assert!(matches!(
            err,
            ViewError::BadField {
                field: "content",
                ..
            }
        ));
    }

    #[test]
    fn a_non_table_child_is_rejected() {
        let err = convert_src(r#"return { kind = "list", children = { 7 } }"#).unwrap_err();
        assert_eq!(
            err,
            ViewError::NotANode {
                got: "integer".to_string()
            }
        );
    }

    #[test]
    fn children_of_the_wrong_type_are_rejected() {
        let err = convert_src(r#"return { kind = "list", children = "nope" }"#).unwrap_err();
        assert!(matches!(
            err,
            ViewError::BadField {
                field: "children",
                ..
            }
        ));
    }
}
