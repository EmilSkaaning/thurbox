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

use crate::session::motion::{Motion, DEFAULT_FPS, MAX_FPS, MAX_FRAMES, MIN_FPS, MIN_FRAMES};
use crate::session::view_tree::{
    sanitize_text, StyleToken, TextStyle, ViewNode, MAX_DEPTH, MAX_NODES,
};

/// The motion kinds this host evaluates, for the error a plugin gets when it
/// names another one. `docs/v2/FEATURES-Animation.md` §2 specifies more; a
/// named rejection is how a plugin author learns which ones exist here rather
/// than watching a declaration render as a still image.
const KNOWN_MOTION_KINDS: &[&str] = &["cycle"];

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
    /// The motion's `kind` is not one this host evaluates.
    UnknownMotionKind(String),
    /// The motion declared too few or too many frames. Out of range is an
    /// error rather than a clamp: a plugin that meant to animate should learn
    /// its declaration was malformed, not silently get a different animation.
    MotionFrameCount {
        /// How many frames arrived.
        got: usize,
    },
    /// The node declaring motion also declared its own content. The frames
    /// *are* the node's content, so keeping both would mean silently dropping
    /// one of them.
    MotionOverContent {
        /// The node kind that carried both.
        kind: String,
    },
    /// A gauge's percentage was not a finite number. Refused rather than
    /// clamped: NaN or an infinity means the plugin's own arithmetic went wrong,
    /// and a bar silently drawn at 0% would hide that.
    NonFinite {
        /// The node kind that carried it.
        kind: String,
        /// The field that was not finite.
        field: &'static str,
    },
    /// A `line` child has no intrinsic width, so there is no honest column to
    /// lay it out at. Named rather than dropped: a plugin author who put a
    /// column in a line wrote a layout they meant, and a silently missing node
    /// reads as the host losing content.
    NotInlineable {
        /// The kind that cannot be laid out inline.
        kind: &'static str,
    },
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
            ViewError::UnknownMotionKind(k) => write!(
                f,
                "unknown motion kind `{k}`; this host evaluates {}",
                KNOWN_MOTION_KINDS.join(", ")
            ),
            ViewError::MotionFrameCount { got } => write!(
                f,
                "motion declared {got} frames; it must declare between \
                 {MIN_FRAMES} and {MAX_FRAMES}"
            ),
            ViewError::MotionOverContent { kind } => write!(
                f,
                "view node `{kind}` declares motion and its own content; the \
                 motion's frames are the content"
            ),
            ViewError::NonFinite { kind, field } => write!(
                f,
                "view node `{kind}` field `{field}` must be a finite number"
            ),
            ViewError::NotInlineable { kind } => write!(
                f,
                "view node `{kind}` cannot sit in a `line`; a line holds only \
                 text, a motion, or another line"
            ),
        }
    }
}

impl std::error::Error for ViewError {}

/// Convert a plugin's render result into a view tree.
pub fn from_lua(value: &Value) -> Result<ViewNode, ViewError> {
    let mut budget = MAX_NODES;
    let mut path = Vec::new();
    convert(value, 1, &mut budget, &mut path)
}

/// Walk one node.
///
/// `depth` starts at 1 for the root. `budget` counts down the remaining node
/// allowance across the whole tree, so breadth is bounded as well as depth —
/// a flat table of a million children would otherwise pass a depth check.
/// `path` is the node's child-index path from the root, which is the fallback
/// identity for a motion on a node that declared no `id`.
fn convert(
    value: &Value,
    depth: usize,
    budget: &mut usize,
    path: &mut Vec<usize>,
) -> Result<ViewNode, ViewError> {
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

    // A node declaring motion becomes a motion node whatever its kind: the
    // frames are what gets drawn, so the kind's own layout never applies.
    if let Ok(Value::Table(decl)) = table.get::<Value>("motion") {
        return convert_motion(table, &decl, &kind, depth, budget, path);
    }

    match kind.as_str() {
        "text" => convert_text(table, &kind),
        "row" => Ok(ViewNode::Row(convert_children(
            table, &kind, depth, budget, path,
        )?)),
        "column" => Ok(ViewNode::Column(convert_children(
            table, &kind, depth, budget, path,
        )?)),
        "list" => Ok(ViewNode::List(convert_children(
            table, &kind, depth, budget, path,
        )?)),
        "line" => {
            let runs = convert_children(table, &kind, depth, budget, path)?;
            // Asked of the converted child rather than decided while walking
            // it: a motion's frames come back from the ordinary walk, which has
            // no idea a line is above it.
            if let Some(kind) = runs.iter().find_map(ViewNode::first_non_inlineable) {
                return Err(ViewError::NotInlineable { kind });
            }
            Ok(ViewNode::Line(runs))
        }
        "paragraph" => {
            let runs = convert_children(table, &kind, depth, budget, path)?;
            // Same admissibility rule as a line, asked of the converted child
            // for the same reason: a motion's frames come back from the ordinary
            // walk, which has no idea a paragraph is above it.
            if let Some(kind) = runs.iter().find_map(ViewNode::first_non_inlineable) {
                return Err(ViewError::NotInlineable { kind });
            }
            Ok(ViewNode::Paragraph(runs))
        }
        "divider" => Ok(ViewNode::Divider),
        "gauge" => convert_gauge(table, &kind),
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

/// Convert a `gauge` node.
///
/// The label is required — an unlabelled bar tells a reader nothing — while the
/// suffix is optional, since the host draws the rounded percentage without one.
fn convert_gauge(table: &Table, kind: &str) -> Result<ViewNode, ViewError> {
    let label = match table.get::<Value>("label") {
        Ok(Value::String(s)) => s.to_string_lossy().to_string(),
        Ok(Value::Nil) => {
            return Err(ViewError::MissingField {
                kind: kind.to_string(),
                field: "label",
            })
        }
        _ => {
            return Err(ViewError::BadField {
                kind: kind.to_string(),
                field: "label",
                expected: "a string",
            })
        }
    };

    let percent = match table.get::<Value>("percent") {
        Ok(Value::Number(n)) => n,
        Ok(Value::Integer(n)) => n as f64,
        Ok(Value::Nil) => {
            return Err(ViewError::MissingField {
                kind: kind.to_string(),
                field: "percent",
            })
        }
        _ => {
            return Err(ViewError::BadField {
                kind: kind.to_string(),
                field: "percent",
                expected: "a number",
            })
        }
    };
    if !percent.is_finite() {
        return Err(ViewError::NonFinite {
            kind: kind.to_string(),
            field: "percent",
        });
    }

    let suffix = match table.get::<Value>("suffix") {
        Ok(Value::String(s)) => Some(s.to_string_lossy().to_string()),
        Ok(Value::Nil) | Err(_) => None,
        Ok(_) => {
            return Err(ViewError::BadField {
                kind: kind.to_string(),
                field: "suffix",
                expected: "a string",
            })
        }
    };

    // Clamping is the renderer's, not conversion's: the node records what the
    // plugin asked for, and the paint decides what fits.
    Ok(ViewNode::gauge(label, percent as f32, suffix))
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

    // Emphasis is opt-in per attribute: anything that is not the boolean `true`
    // — absent, `false`, or a value of another type — leaves the attribute off,
    // so a misspelled field cannot silently style a run.
    let flag = |name: &str| matches!(table.get::<Value>(name), Ok(Value::Boolean(true)));

    Ok(ViewNode::Text {
        content: sanitize_text(&content),
        style: TextStyle {
            token,
            bold: flag("bold"),
            dim: flag("dim"),
            underline: flag("underline"),
        },
    })
}

/// Convert a node's `motion` declaration into a [`ViewNode::Motion`].
///
/// The node's identity is resolved here, once, and stored in the node: the
/// kernel's epoch table and the renderer both read it, and deriving it twice
/// from two different tree walks is how they would come to disagree.
fn convert_motion(
    node: &Table,
    decl: &Table,
    kind: &str,
    depth: usize,
    budget: &mut usize,
    path: &mut Vec<usize>,
) -> Result<ViewNode, ViewError> {
    // The frames are the node's content; carrying both would mean silently
    // dropping one.
    if !matches!(node.get::<Value>("children"), Ok(Value::Nil) | Err(_))
        || !matches!(node.get::<Value>("content"), Ok(Value::Nil) | Err(_))
    {
        return Err(ViewError::MotionOverContent {
            kind: kind.to_string(),
        });
    }

    let motion_kind = match decl.get::<Value>("kind") {
        Ok(Value::String(s)) => s.to_string_lossy().to_string(),
        Ok(Value::Nil) | Err(_) => {
            return Err(ViewError::MissingField {
                kind: "motion".to_string(),
                field: "kind",
            })
        }
        Ok(_) => {
            return Err(ViewError::BadField {
                kind: "motion".to_string(),
                field: "kind",
                expected: "a string",
            })
        }
    };
    if motion_kind != "cycle" {
        return Err(ViewError::UnknownMotionKind(motion_kind));
    }

    let frames_table = match decl.get::<Value>("frames") {
        Ok(Value::Table(t)) => t,
        Ok(Value::Nil) | Err(_) => {
            return Err(ViewError::MissingField {
                kind: "motion".to_string(),
                field: "frames",
            })
        }
        Ok(_) => {
            return Err(ViewError::BadField {
                kind: "motion".to_string(),
                field: "frames",
                expected: "an array",
            })
        }
    };

    let mut frames = Vec::new();
    for (i, pair) in frames_table.sequence_values::<Value>().enumerate() {
        let value = pair.map_err(|_| ViewError::BadField {
            kind: "motion".to_string(),
            field: "frames",
            expected: "an array",
        })?;
        // A frame is an ordinary subtree, walked under the same depth and node
        // budget as any child — which is what makes a motion's cost knowable
        // at push time.
        path.push(i);
        let converted = convert(&value, depth + 1, budget, path);
        path.pop();
        frames.push(converted?);
    }
    if !(MIN_FRAMES..=MAX_FRAMES).contains(&frames.len()) {
        return Err(ViewError::MotionFrameCount { got: frames.len() });
    }

    // A declared rate is clamped rather than rejected: unlike a malformed
    // frame list, an out-of-range rate has an obviously correct reading, and
    // the cap exists to bound the kernel regardless of what was asked for.
    let fps = match decl.get::<Value>("fps") {
        Ok(Value::Integer(n)) => n.clamp(i64::from(MIN_FPS), i64::from(MAX_FPS)) as u8,
        Ok(Value::Number(n)) => n.clamp(f64::from(MIN_FPS), f64::from(MAX_FPS)) as u8,
        Ok(Value::Nil) | Err(_) => DEFAULT_FPS,
        Ok(_) => {
            return Err(ViewError::BadField {
                kind: "motion".to_string(),
                field: "fps",
                expected: "a number",
            })
        }
    };

    // Defaults the way an author who omitted it would expect: an animation
    // loops until its node goes away.
    let repeat = !matches!(decl.get::<Value>("loop"), Ok(Value::Boolean(false)));

    let declared_id = match node.get::<Value>("id") {
        Ok(Value::String(s)) => Some(sanitize_text(&s.to_string_lossy())),
        _ => None,
    };
    let (key, keyed_by_id) = match declared_id {
        Some(id) if !id.is_empty() => (id, true),
        // Structural fallback: correct only while the tree shape is stable,
        // which is why the host reports it rather than quietly restarting the
        // animation the first time a sibling appears.
        _ => (structural_key(path), false),
    };

    Ok(ViewNode::Motion {
        key,
        keyed_by_id,
        motion: Motion::cycle(frames, fps, repeat),
    })
}

/// The identity a motion gets when its node declared no `id`: its child-index
/// path from the root. `@` prefixes it so it can never collide with a declared
/// id, which is sanitized text.
fn structural_key(path: &[usize]) -> String {
    let mut key = String::from("@");
    for (i, step) in path.iter().enumerate() {
        if i > 0 {
            key.push('.');
        }
        key.push_str(&step.to_string());
    }
    key
}

/// Convert a container's `children` array.
fn convert_children(
    table: &Table,
    kind: &str,
    depth: usize,
    budget: &mut usize,
    path: &mut Vec<usize>,
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
    for (i, pair) in children.sequence_values::<Value>().enumerate() {
        let value = pair.map_err(|_| ViewError::BadField {
            kind: kind.to_string(),
            field: "children",
            expected: "an array",
        })?;
        path.push(i);
        let converted = convert(&value, depth + 1, budget, path);
        path.pop();
        out.push(converted?);
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
    fn a_gauge_converts() {
        let node =
            convert_src(r#"return { kind = "gauge", label = "CPU", percent = 42.5 }"#).unwrap();
        assert_eq!(node, ViewNode::gauge("CPU", 42.5, None));

        let with_suffix = convert_src(
            r#"return { kind = "gauge", label = "RAM", percent = 50, suffix = "8/16 GB" }"#,
        )
        .unwrap();
        assert_eq!(
            with_suffix,
            ViewNode::gauge("RAM", 50.0, Some("8/16 GB".to_string()))
        );
    }

    #[test]
    fn a_gauge_needs_a_label_and_a_percentage() {
        assert_eq!(
            convert_src(r#"return { kind = "gauge", percent = 1 }"#),
            Err(ViewError::MissingField {
                kind: "gauge".to_string(),
                field: "label"
            })
        );
        assert_eq!(
            convert_src(r#"return { kind = "gauge", label = "x" }"#),
            Err(ViewError::MissingField {
                kind: "gauge".to_string(),
                field: "percent"
            })
        );
        assert_eq!(
            convert_src(r#"return { kind = "gauge", label = "x", percent = "half" }"#),
            Err(ViewError::BadField {
                kind: "gauge".to_string(),
                field: "percent",
                expected: "a number"
            })
        );
    }

    #[test]
    fn a_gauge_percentage_must_be_finite() {
        // Refused rather than clamped: a NaN means the plugin's own arithmetic
        // went wrong, and a bar silently drawn at 0% would hide that.
        let err = convert_src(r#"return { kind = "gauge", label = "x", percent = 0/0 }"#)
            .expect_err("a NaN percentage is refused");
        assert_eq!(
            err,
            ViewError::NonFinite {
                kind: "gauge".to_string(),
                field: "percent"
            }
        );
        assert!(err.to_string().contains("finite"), "{err}");

        assert!(convert_src(r#"return { kind = "gauge", label = "x", percent = 1/0 }"#).is_err());
    }

    #[test]
    fn a_paragraph_converts_and_admits_only_inline_children() {
        let node = convert_src(
            r#"return { kind = "paragraph", children = { { kind = "text", content = "hi" } } }"#,
        )
        .unwrap();
        assert_eq!(node, ViewNode::Paragraph(vec![ViewNode::text("hi")]));

        // Same rule as a line, for the same reason: a column has no intrinsic
        // width, so there is no honest column to place it at.
        assert_eq!(
            convert_src(r#"return { kind = "paragraph", children = { { kind = "column" } } }"#),
            Err(ViewError::NotInlineable { kind: "column" })
        );
    }

    #[test]
    fn neither_new_kind_may_sit_inside_a_line() {
        for child in [
            r#"{ kind = "paragraph" }"#,
            r#"{ kind = "gauge", label = "x", percent = 1 }"#,
        ] {
            let src = format!(r#"return {{ kind = "line", children = {{ {child} }} }}"#);
            assert!(
                matches!(convert_src(&src), Err(ViewError::NotInlineable { .. })),
                "{child} must be refused inside a line"
            );
        }
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
                    bold: true,
                    ..TextStyle::default()
                }
            )
        );
    }

    /// Each emphasis is its own field and each is opt-in: a plugin that declares
    /// one must not get another, and a field it did not declare must be off.
    #[test]
    fn each_emphasis_converts_independently() {
        let node =
            convert_src(r#"return { kind = "text", content = "hi", style = "muted", dim = true }"#)
                .unwrap();
        assert_eq!(
            node,
            ViewNode::styled(
                "hi",
                TextStyle {
                    token: Some(StyleToken::Muted),
                    dim: true,
                    ..TextStyle::default()
                }
            )
        );

        let all = convert_src(
            r#"return { kind = "text", content = "hi", bold = true, dim = true,
                        underline = true }"#,
        )
        .unwrap();
        assert_eq!(
            all,
            ViewNode::styled(
                "hi",
                TextStyle {
                    token: None,
                    bold: true,
                    dim: true,
                    underline: true,
                }
            )
        );
    }

    /// Anything that is not the boolean `true` leaves the attribute off, so a
    /// misspelled or mistyped value cannot silently style a run.
    #[test]
    fn a_non_boolean_emphasis_is_off_rather_than_on() {
        for decl in [
            r#"underline = false"#,
            r#"underline = "yes""#,
            r#"underline = 1"#,
            r#"dim = {}"#,
        ] {
            let node = convert_src(&format!(
                r#"return {{ kind = "text", content = "x", {decl} }}"#
            ))
            .unwrap();
            assert_eq!(node, ViewNode::text("x"), "{decl}");
        }
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
    fn a_line_of_styled_runs_converts() {
        let node = convert_src(
            r#"return { kind = "line", children = {
                 { kind = "text", content = "Name: ", style = "muted" },
                 { kind = "text", content = "demo", bold = true },
               } }"#,
        )
        .unwrap();
        assert_eq!(
            node,
            ViewNode::Line(vec![
                ViewNode::styled(
                    "Name: ",
                    TextStyle {
                        token: Some(StyleToken::Muted),
                        ..TextStyle::default()
                    }
                ),
                ViewNode::styled(
                    "demo",
                    TextStyle {
                        token: None,
                        bold: true,
                        ..TextStyle::default()
                    }
                ),
            ])
        );
    }

    #[test]
    fn an_empty_line_converts() {
        assert_eq!(
            convert_src(r#"return { kind = "line" }"#).unwrap(),
            ViewNode::Line(Vec::new())
        );
    }

    #[test]
    fn a_line_refuses_a_child_with_no_intrinsic_width() {
        // Each of these would have to be measured against the area it was given
        // rather than against its own content.
        for kind in ["column", "list", "row", "divider", "spacer"] {
            let src =
                format!(r#"return {{ kind = "line", children = {{ {{ kind = "{kind}" }} }} }}"#);
            let err = convert_src(&src).unwrap_err();
            assert_eq!(err, ViewError::NotInlineable { kind });
            assert!(err.to_string().contains(kind), "{err}");
        }
    }

    #[test]
    fn a_line_accepts_a_motion_whose_frames_are_runs() {
        let node = convert_src(
            r#"return { kind = "line", children = {
                 { kind = "motion", id = "dot", motion = { kind = "cycle", frames = {
                     { kind = "text", content = "-" },
                     { kind = "text", content = "\\" },
                 } } },
                 { kind = "text", content = " working" },
               } }"#,
        )
        .unwrap();
        match node {
            ViewNode::Line(runs) => {
                assert_eq!(runs.len(), 2);
                assert!(matches!(runs[0], ViewNode::Motion { .. }));
            }
            other => panic!("expected a line, got {other:?}"),
        }
    }

    #[test]
    fn a_motion_cannot_smuggle_a_column_into_a_line() {
        // The restriction has to survive a frame, or wrapping a column in a
        // two-frame animation would be a way around it.
        let err = convert_src(
            r#"return { kind = "line", children = {
                 { kind = "motion", motion = { kind = "cycle", frames = {
                     { kind = "text", content = "." },
                     { kind = "column", children = {} },
                 } } },
               } }"#,
        )
        .unwrap_err();
        assert_eq!(err, ViewError::NotInlineable { kind: "column" });
    }

    #[test]
    fn a_line_may_nest_a_line() {
        let node = convert_src(
            r#"return { kind = "line", children = {
                 { kind = "line", children = { { kind = "text", content = "a" } } },
                 { kind = "text", content = "b" },
               } }"#,
        )
        .unwrap();
        assert_eq!(
            node,
            ViewNode::Line(vec![
                ViewNode::Line(vec![ViewNode::text("a")]),
                ViewNode::text("b"),
            ])
        );
    }

    #[test]
    fn a_lines_runs_count_against_the_node_budget() {
        // One line plus MAX_NODES runs is one node too many; the line does not
        // give a plugin a way around the budget.
        let runs = (0..MAX_NODES)
            .map(|_| r#"{ kind = "text", content = "x" }"#)
            .collect::<Vec<_>>()
            .join(",");
        let err = convert_src(&format!(
            r#"return {{ kind = "line", children = {{ {runs} }} }}"#
        ))
        .unwrap_err();
        assert_eq!(err, ViewError::TooManyNodes);
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

    /// A cycle: two frames of text under a declared id.
    const CYCLE: &str = r#"return {
        kind = "column",
        id = "thinking",
        motion = {
            kind = "cycle",
            fps = 8,
            frames = {
                { kind = "text", content = "one" },
                { kind = "text", content = "two" },
            },
        },
    }"#;

    #[test]
    fn a_cycle_converts_and_keys_on_the_declared_id() {
        let node = convert_src(CYCLE).unwrap();
        match node {
            ViewNode::Motion {
                key,
                keyed_by_id,
                motion,
            } => {
                assert_eq!(key, "thinking");
                assert!(keyed_by_id);
                assert_eq!(motion.fps, 8);
                assert_eq!(motion.frames().len(), 2);
                assert!(motion.repeats(), "loops unless told otherwise");
            }
            other => panic!("expected a motion node, got {other:?}"),
        }
    }

    #[test]
    fn an_id_less_motion_falls_back_to_its_position() {
        let node = convert_src(
            r#"return { kind = "column", children = {
                { kind = "text", content = "first" },
                { kind = "column", motion = { kind = "cycle", frames = {
                    { kind = "text", content = "a" },
                    { kind = "text", content = "b" },
                }}},
            }}"#,
        )
        .unwrap();
        let ViewNode::Column(children) = &node else {
            panic!("expected a column, got {node:?}");
        };
        match &children[1] {
            ViewNode::Motion {
                key, keyed_by_id, ..
            } => {
                assert_eq!(key, "@1", "the second child of the root");
                assert!(!keyed_by_id);
            }
            other => panic!("expected a motion node, got {other:?}"),
        }
    }

    #[test]
    fn an_unknown_motion_kind_names_the_ones_that_exist() {
        let err = convert_src(
            r#"return { kind = "column", motion = { kind = "marquee", frames = {
                { kind = "text", content = "a" },
                { kind = "text", content = "b" },
            }}}"#,
        )
        .unwrap_err();
        assert_eq!(err, ViewError::UnknownMotionKind("marquee".to_string()));
        assert!(err.to_string().contains("cycle"), "{err}");
    }

    #[test]
    fn too_few_frames_is_rejected_rather_than_rendered_static() {
        let err = convert_src(
            r#"return { kind = "column", motion = { kind = "cycle", frames = {
                { kind = "text", content = "only" },
            }}}"#,
        )
        .unwrap_err();
        assert_eq!(err, ViewError::MotionFrameCount { got: 1 });
    }

    #[test]
    fn too_many_frames_is_rejected_rather_than_truncated() {
        let src = format!(
            r#"
            local frames = {{}}
            for i = 1, {} do
                frames[i] = {{ kind = "text", content = "x" }}
            end
            return {{ kind = "column", motion = {{ kind = "cycle", frames = frames }} }}
            "#,
            MAX_FRAMES + 1
        );
        assert_eq!(
            convert_src(&src).unwrap_err(),
            ViewError::MotionFrameCount {
                got: MAX_FRAMES + 1
            }
        );
    }

    #[test]
    fn a_declared_rate_is_clamped_to_the_per_pane_cap() {
        let fast = convert_src(
            r#"return { kind = "column", motion = { kind = "cycle", fps = 240, frames = {
                { kind = "text", content = "a" }, { kind = "text", content = "b" },
            }}}"#,
        )
        .unwrap();
        let slow = convert_src(
            r#"return { kind = "column", motion = { kind = "cycle", fps = 0, frames = {
                { kind = "text", content = "a" }, { kind = "text", content = "b" },
            }}}"#,
        )
        .unwrap();
        let fps = |n: &ViewNode| match n {
            ViewNode::Motion { motion, .. } => motion.fps,
            other => panic!("expected a motion node, got {other:?}"),
        };
        assert_eq!(fps(&fast), MAX_FPS);
        assert_eq!(fps(&slow), MIN_FPS);
    }

    #[test]
    fn an_omitted_rate_takes_the_default() {
        let node = convert_src(
            r#"return { kind = "column", motion = { kind = "cycle", frames = {
                { kind = "text", content = "a" }, { kind = "text", content = "b" },
            }}}"#,
        )
        .unwrap();
        match node {
            ViewNode::Motion { motion, .. } => assert_eq!(motion.fps, DEFAULT_FPS),
            other => panic!("expected a motion node, got {other:?}"),
        }
    }

    #[test]
    fn a_motion_node_may_not_also_carry_its_own_content() {
        // The frames are the content; keeping both would silently drop one.
        let err = convert_src(
            r#"return { kind = "column", children = { { kind = "text", content = "x" } },
               motion = { kind = "cycle", frames = {
                   { kind = "text", content = "a" }, { kind = "text", content = "b" },
               }}}"#,
        )
        .unwrap_err();
        assert!(
            matches!(err, ViewError::MotionOverContent { .. }),
            "{err:?}"
        );
    }

    #[test]
    fn frames_count_against_the_tree_budget() {
        let src = format!(
            r#"
            local frames = {{}}
            for i = 1, 2 do
                local kids = {{}}
                for j = 1, {} do
                    kids[j] = {{ kind = "text", content = "x" }}
                end
                frames[i] = {{ kind = "list", children = kids }}
            end
            return {{ kind = "column", motion = {{ kind = "cycle", frames = frames }} }}
            "#,
            MAX_NODES
        );
        assert_eq!(convert_src(&src).unwrap_err(), ViewError::TooManyNodes);
    }

    #[test]
    fn a_motion_missing_its_frames_is_named() {
        let err =
            convert_src(r#"return { kind = "column", motion = { kind = "cycle" } }"#).unwrap_err();
        assert_eq!(
            err,
            ViewError::MissingField {
                kind: "motion".to_string(),
                field: "frames",
            }
        );
    }

    /// `ui.cycle` is the constructor a plugin actually calls; it must build a
    /// table the converter accepts, so the two cannot drift.
    #[test]
    fn the_ui_cycle_constructor_produces_a_convertible_node() {
        let lua = Lua::new();
        let module = crate::plugin::capabilities::build_module_table(
            &lua,
            "demo",
            &crate::plugin::capabilities::GrantedCapabilities::from_manifest(
                &std::collections::BTreeSet::new(),
            ),
            None,
        )
        .expect("module builds");
        lua.globals().set("thurbox", module).unwrap();

        let value: Value = lua
            .load(
                r#"return thurbox.ui.cycle("spinner", {
                    thurbox.ui.text("a"), thurbox.ui.text("b"),
                }, 12)"#,
            )
            .eval()
            .expect("chunk evaluates");
        match from_lua(&value).unwrap() {
            ViewNode::Motion {
                key,
                keyed_by_id,
                motion,
            } => {
                assert_eq!(key, "spinner");
                assert!(keyed_by_id);
                assert_eq!(motion.fps, 12);
                assert_eq!(motion.frames().len(), 2);
            }
            other => panic!("expected a motion node, got {other:?}"),
        }
    }

    #[test]
    fn a_non_repeating_cycle_is_declared_with_loop_false() {
        let node = convert_src(
            r#"return { kind = "column", motion = { kind = "cycle", loop = false, frames = {
                { kind = "text", content = "a" }, { kind = "text", content = "b" },
            }}}"#,
        )
        .unwrap();
        match node {
            ViewNode::Motion { motion, .. } => assert!(!motion.repeats()),
            other => panic!("expected a motion node, got {other:?}"),
        }
    }
}
