//! The recorded form of a view tree: one line per node, readable by a reviewer.
//!
//! This is the oracle that outlives the native builder, so its two properties are
//! load-bearing in opposite directions.
//!
//! **Legible**, because `{:#?}` is not. The full case's structural dump runs to
//! thousands of lines, most of them style fields at their defaults; nobody can
//! tell a correct one from a subtly wrong one at that size, so every future
//! update would be a rubber stamp — and a rubber-stamped expectation records what
//! the code last did rather than what the pane should show. Here a lost bold or a
//! changed colour role is one visible word on one line.
//!
//! **Exhaustive**, because compactness is bought by omitting, and an omission is a
//! hole in the oracle. Every `ViewNode` variant and every [`TextStyle`] field is
//! destructured *by name* below, with no `..` rest pattern and no wildcard arm:
//! adding a field to the view tree fails to compile here until this decides how to
//! print it. The compiler is what keeps the format honest, not a reviewer's memory.
//!
//! Colour roles print through `as_str`, the wire name a plugin uses, so a token
//! added to the IR has to be spelled in the library's own exhaustive match first.
//!
//! ## Why this one is shared, when the gates' helpers are copied
//!
//! `tests/tasks_pane_input_gap.rs` and `tests/global_search_pane_gap.rs`
//! deliberately duplicate their source-reading helpers, and a subdirectory of
//! `tests/` is not a test target — so a shared module was available to them and
//! was not taken. The distinction is what the duplicated thing *is*. Two readers
//! of source text answer independently and drift between them is harmless. Two
//! recorders are two definitions of what an expectation **contains**, and drift
//! between them is precisely the failure ADR-42 exists to prevent: a fact one
//! pane's oracle silently stops constraining. The exhaustiveness above is worth
//! as much as the number of copies of it, so there is one.
use thurbox::session::motion::{Motion, MotionKind};
use thurbox::session::view_tree::{TextStyle, ViewNode};

/// The whole tree, newline-separated, with a trailing newline per line.
pub fn tree(node: &ViewNode) -> String {
    let mut out = String::new();
    write_node(node, 0, &mut out);
    out
}

/// Assert that a plugin's tree records the same lines as the native pane's, and
/// fail with the first line that moved.
///
/// The legible half of a pane's oracle, and it exists for the same reason the
/// recording is compact at all: a bare `assert_eq!` on two trees prints two
/// thousand-character structural dumps on one line each, so the reader of a
/// failure has to diff them by eye — which is how a real divergence gets waved
/// through as "probably the fixture". Here a lost bold is one line.
///
/// Lines are compared **by position**, with no attempt to realign after an
/// insertion. That is deliberate: an inserted node shifts everything below it, and
/// the honest report of that is "the trees stopped agreeing at line N" plus the
/// two line counts. A cleverer alignment would name a later, prettier difference
/// and bury the actual one.
///
/// Unused by a **handed-over** pane's oracle, which has no native tree left to
/// compare against and asserts the recording alone (ADR-50) — and this module is
/// compiled separately into each oracle's test binary, so the first such handover
/// made it dead code in exactly one of them. Allowed rather than deleted: the five
/// panes still drawn natively use it, and the day the last one is handed over is the
/// day to remove it.
#[allow(dead_code)]
pub fn assert_matches(case: &str, plugin: &ViewNode, native: &ViewNode) {
    let (plugin, native) = (tree(plugin), tree(native));
    if plugin == native {
        return;
    }
    let (mut got, mut want) = (plugin.lines(), native.lines());
    let mut line = 0usize;
    let report = loop {
        line += 1;
        match (got.next(), want.next()) {
            (Some(a), Some(b)) if a == b => continue,
            (a, b) => {
                break format!(
                    "line {line}:\n  native: {}\n  plugin: {}",
                    b.unwrap_or("<end of tree>"),
                    a.unwrap_or("<end of tree>"),
                )
            }
        }
    };
    panic!(
        "the plugin does not reproduce the recorded pane for `{case}`\n{report}\n\
         \nnative ({} lines):\n{native}\nplugin ({} lines):\n{plugin}",
        native.lines().count(),
        plugin.lines().count(),
    );
}

fn write_node(node: &ViewNode, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    // Exhaustive by construction: no `..`, no `_` arm. See the module note.
    match node {
        ViewNode::Text { content, style } => {
            out.push_str(&format!("{pad}text {content:?}{}\n", style_facts(style)));
        }
        ViewNode::Row(children) => {
            out.push_str(&format!("{pad}row\n"));
            write_children(children, depth, out);
        }
        ViewNode::Line(children) => {
            out.push_str(&format!("{pad}line\n"));
            write_children(children, depth, out);
        }
        ViewNode::Center(children) => {
            out.push_str(&format!("{pad}center\n"));
            write_children(children, depth, out);
        }
        ViewNode::Paragraph(children) => {
            out.push_str(&format!("{pad}paragraph\n"));
            write_children(children, depth, out);
        }
        ViewNode::Column(children) => {
            out.push_str(&format!("{pad}column\n"));
            write_children(children, depth, out);
        }
        ViewNode::List {
            children,
            selected,
            scrollbar,
        } => {
            let cursor = match selected {
                Some(i) => i.to_string(),
                None => "none".to_string(),
            };
            out.push_str(&format!(
                "{pad}list cursor={cursor} scrollbar={scrollbar}\n"
            ));
            write_children(children, depth, out);
        }
        ViewNode::Divider => out.push_str(&format!("{pad}divider\n")),
        ViewNode::Gauge {
            label,
            percent,
            suffix,
        } => {
            // The float prints through `{:?}`, which round-trips exactly:
            // `Percent` compares by bit pattern, so a lossy decimal here
            // would let two gauges the host draws differently record the
            // same, narrowing the oracle it exists to be.
            let suffix = match suffix {
                Some(s) => format!("{s:?}"),
                None => "none".to_string(),
            };
            out.push_str(&format!(
                "{pad}gauge {label:?} percent={:?} suffix={suffix}\n",
                percent.get()
            ));
        }
        ViewNode::Fill { glyph, style } => {
            out.push_str(&format!("{pad}fill {glyph:?}{}\n", style_facts(style)));
        }
        ViewNode::Spacer { lines } => out.push_str(&format!("{pad}spacer lines={lines}\n")),
        ViewNode::Motion {
            key,
            keyed_by_id,
            motion,
        } => {
            let Motion { fps, kind } = motion;
            match kind {
                MotionKind::Cycle { frames, repeat } => {
                    out.push_str(&format!(
                        "{pad}motion cycle key={key:?} keyed_by_id={keyed_by_id} \
                         fps={fps} repeat={repeat}\n"
                    ));
                    write_children(frames, depth, out);
                }
            }
        }
    }
}

fn write_children(children: &[ViewNode], depth: usize, out: &mut String) {
    for child in children {
        write_node(child, depth + 1, out);
    }
}

/// A style's non-default facts, space-prefixed; empty for the default style.
///
/// Absence means default, and the exhaustive destructuring is what makes that
/// safe: every fact is printed when it is set, so "nothing printed" cannot
/// hide one this format forgot about.
fn style_facts(style: &TextStyle) -> String {
    let TextStyle {
        token,
        bold,
        dim,
        underline,
        selected,
        tint,
        ellipsize,
    } = style;
    let mut facts: Vec<String> = Vec::new();
    if let Some(token) = token {
        facts.push(token.as_str().to_string());
    }
    if *bold {
        facts.push("bold".to_string());
    }
    if *dim {
        facts.push("dim".to_string());
    }
    if *underline {
        facts.push("underline".to_string());
    }
    if *selected {
        facts.push("selected".to_string());
    }
    if let Some(tint) = tint {
        facts.push(format!("tint={}", tint.as_str()));
    }
    if *ellipsize {
        facts.push("ellipsize".to_string());
    }
    if facts.is_empty() {
        String::new()
    } else {
        format!(" {}", facts.join(" "))
    }
}
