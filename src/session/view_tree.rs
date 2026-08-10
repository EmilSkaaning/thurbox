//! The declarative view tree a plugin returns to describe its pane.
//!
//! Pure data (no local crate imports beyond `super`), matching the `session/`
//! architecture rule. [`crate::plugin`] converts a plugin's Lua value into
//! these types; [`crate::ui`] renders them. That split is what keeps `ui` free
//! of any path back to a VM — the renderer literally cannot call a plugin,
//! because the type it renders has no reference to one. It mirrors how
//! [`crate::session::review`] already splits diff types from `git` and `ui`.
//!
//! The catalog is deliberately small: it is the set thurbox's own panes need,
//! not a general drawing API. Widening it later is additive; narrowing it once
//! plugins depend on it would not be.

/// Deepest nesting a view tree may have.
///
/// Bounds the conversion walk, so a self-referential Lua table terminates here
/// rather than looping forever.
pub const MAX_DEPTH: usize = 32;

/// Most nodes one tree may contain.
pub const MAX_NODES: usize = 4096;

/// Longest text a single node may carry, in characters.
pub const MAX_TEXT_LEN: usize = 4096;

/// A named colour role, resolved against the active thurbox theme at paint
/// time.
///
/// Plugins style by token rather than by colour on purpose: thurbox ships 36
/// palettes, eight of them light, so a plugin naming an RGB value would be
/// unreadable on many of them and would stop matching the instant a user
/// switched theme. A closed token set makes theme-following the only option a
/// plugin has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StyleToken {
    /// The theme's accent — headings, selected rows.
    Accent,
    /// De-emphasised secondary text.
    Muted,
    /// Something went wrong.
    Danger,
    /// Something succeeded.
    Success,
    /// Something needs attention but is not an error.
    Warning,
}

impl StyleToken {
    /// The wire name a plugin uses.
    pub fn as_str(self) -> &'static str {
        match self {
            StyleToken::Accent => "accent",
            StyleToken::Muted => "muted",
            StyleToken::Danger => "danger",
            StyleToken::Success => "success",
            StyleToken::Warning => "warning",
        }
    }

    /// Every token the host defines.
    pub fn all() -> &'static [StyleToken] {
        &[
            StyleToken::Accent,
            StyleToken::Muted,
            StyleToken::Danger,
            StyleToken::Success,
            StyleToken::Warning,
        ]
    }

    /// Parse a wire name, or `None` if the host does not define it.
    pub fn parse(s: &str) -> Option<StyleToken> {
        StyleToken::all().iter().copied().find(|t| t.as_str() == s)
    }
}

/// Text styling flags a node may carry alongside its colour token.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct TextStyle {
    /// Colour role; `None` renders in the theme's default foreground.
    pub token: Option<StyleToken>,
    /// Render bold.
    pub bold: bool,
}

/// One node in a plugin's view tree.
///
/// Text is the only content node; everything else arranges children. A tree is
/// static in content — it changes only when the plugin returns a different one
/// — but a [`ViewNode::Motion`] node lets the *kernel* choose which of several
/// supplied subtrees to draw, from its own clock.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ViewNode {
    /// A run of text. Already sanitized and truncated by [`sanitize_text`].
    Text {
        /// The content to draw.
        content: String,
        /// How to draw it.
        style: TextStyle,
    },
    /// Children laid out left to right.
    Row(Vec<ViewNode>),
    /// Children packed left to right on **one** row, each at its own intrinsic
    /// width.
    ///
    /// The inline counterpart of [`ViewNode::Row`]: a row *divides* an area
    /// into equal shares, a line *composes* a sentence out of runs. Both exist
    /// because a `label: value` line — the shape most of thurbox's own panes
    /// are built from — needs several styles on one row at natural widths,
    /// which equal shares cannot give and a single [`ViewNode::Text`] cannot
    /// style.
    ///
    /// Only children with an intrinsic width may appear here; see
    /// [`ViewNode::is_inlineable`].
    Line(Vec<ViewNode>),
    /// Children laid out top to bottom.
    Column(Vec<ViewNode>),
    /// Children laid out top to bottom, one per line — a column that reads as
    /// a list to a user, and the shape most panes actually want.
    List(Vec<ViewNode>),
    /// A horizontal rule filling the available width.
    Divider,
    /// Blank vertical space.
    Spacer {
        /// How many lines to leave empty.
        lines: u16,
    },
    /// One of several supplied subtrees, chosen by the kernel's clock.
    ///
    /// The plugin declares this as a `motion` field on any node; the host
    /// lifts it into its own node so a motion's frames are ordinary children
    /// and pay the same depth and node-count bounds as everything else.
    Motion {
        /// Identity for the kernel's phase bookkeeping, derived once at
        /// conversion: the node's declared `id`, or its structural path when
        /// it has none. Deriving it here rather than at each use is what keeps
        /// the epoch table and the renderer from disagreeing about which node
        /// they are talking about.
        key: String,
        /// Whether `key` came from a declared id. A structural key is correct
        /// only while the tree shape is stable, so the host reports when one
        /// is in use.
        keyed_by_id: bool,
        /// What it does and how fast.
        motion: super::motion::Motion,
    },
}

impl ViewNode {
    /// A node's children, or an empty slice for a leaf.
    pub fn children(&self) -> &[ViewNode] {
        match self {
            ViewNode::Row(c) | ViewNode::Line(c) | ViewNode::Column(c) | ViewNode::List(c) => c,
            // A motion's frames are its children: that is what makes them
            // count against the tree budget rather than escaping it.
            ViewNode::Motion { motion, .. } => motion.frames(),
            ViewNode::Text { .. } | ViewNode::Divider | ViewNode::Spacer { .. } => &[],
        }
    }

    /// Total nodes in this subtree, including itself.
    pub fn node_count(&self) -> usize {
        1 + self
            .children()
            .iter()
            .map(ViewNode::node_count)
            .sum::<usize>()
    }

    /// Deepest nesting in this subtree; a leaf is depth 1.
    pub fn depth(&self) -> usize {
        1 + self
            .children()
            .iter()
            .map(ViewNode::depth)
            .max()
            .unwrap_or(0)
    }

    /// Whether this node can be laid out inside a [`ViewNode::Line`].
    ///
    /// A line places each child at the width its content needs, so a child is
    /// admissible exactly when its width follows from its content: a text run,
    /// a nested line, or a motion whose every frame is itself inlineable. A
    /// column, list, divider or spacer has no such width — a divider fills
    /// whatever it is given and a spacer is vertical — so it is refused rather
    /// than measured as zero and silently dropped.
    ///
    /// Asking the finished node, rather than threading an "inside a line" flag
    /// through conversion, is what makes the rule hold through a motion frame:
    /// frames are converted by the ordinary walk, which knows nothing about the
    /// line above it. It also fails safe — a node kind added later is not
    /// inlineable until someone says it is.
    pub fn is_inlineable(&self) -> bool {
        self.first_non_inlineable().is_none()
    }

    /// The kind of the first node in this subtree that cannot be laid out
    /// inline, or `None` when the whole subtree can.
    ///
    /// Returns the *offending* kind rather than this node's own, so the error a
    /// plugin author reads names the column they nested inside an animation
    /// rather than the animation that carried it.
    pub fn first_non_inlineable(&self) -> Option<&'static str> {
        match self {
            ViewNode::Text { .. } => None,
            ViewNode::Line(children) => children.iter().find_map(ViewNode::first_non_inlineable),
            ViewNode::Motion { motion, .. } => motion
                .frames()
                .iter()
                .find_map(ViewNode::first_non_inlineable),
            ViewNode::Row(_)
            | ViewNode::Column(_)
            | ViewNode::List(_)
            | ViewNode::Divider
            | ViewNode::Spacer { .. } => Some(self.kind_name()),
        }
    }

    /// The node kind's wire name, for an error that has to name it.
    pub fn kind_name(&self) -> &'static str {
        match self {
            ViewNode::Text { .. } => "text",
            ViewNode::Row(_) => "row",
            ViewNode::Line(_) => "line",
            ViewNode::Column(_) => "column",
            ViewNode::List(_) => "list",
            ViewNode::Divider => "divider",
            ViewNode::Spacer { .. } => "spacer",
            ViewNode::Motion { .. } => "motion",
        }
    }

    /// Build a plain text node with no styling.
    pub fn text(content: impl Into<String>) -> ViewNode {
        ViewNode::Text {
            content: sanitize_text(&content.into()),
            style: TextStyle::default(),
        }
    }

    /// Build a styled text node.
    pub fn styled(content: impl Into<String>, style: TextStyle) -> ViewNode {
        ViewNode::Text {
            content: sanitize_text(&content.into()),
            style,
        }
    }
}

/// Make a plugin-supplied string safe to draw.
///
/// Control characters are dropped rather than escaped: a plugin emitting an
/// ANSI sequence would otherwise move the cursor, recolour the rest of the
/// frame, or clear the screen — the terminal cannot tell a plugin's bytes from
/// thurbox's. Tabs become spaces so column maths stays predictable. Truncation
/// is on a **character** boundary, since cutting a byte index would split a
/// multi-byte character into invalid UTF-8.
pub fn sanitize_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len().min(MAX_TEXT_LEN));
    for c in raw.chars() {
        if out.chars().count() >= MAX_TEXT_LEN {
            break;
        }
        match c {
            '\t' => out.push_str("    "),
            // Newlines are structural in this model — a plugin splits lines by
            // returning separate nodes, so an embedded newline would silently
            // break the layout the tree describes.
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A motion's frames are reachable as its children, which is the whole
    /// mechanism that makes them pay the tree bounds: were `children()` to hide
    /// them, a plugin could smuggle arbitrarily many nodes past the budget by
    /// declaring them as frames.
    #[test]
    fn a_motions_frames_count_toward_the_tree_bounds() {
        let motion = ViewNode::Motion {
            key: "spinner".to_string(),
            keyed_by_id: true,
            motion: super::super::motion::Motion::cycle(
                vec![
                    ViewNode::text("."),
                    ViewNode::Row(vec![ViewNode::text(".."), ViewNode::text("...")]),
                ],
                8,
                true,
            ),
        };

        assert_eq!(motion.children().len(), 2, "frames are the children");
        // 1 motion + frame one + (frame two + its two texts).
        assert_eq!(motion.node_count(), 5);
        assert_eq!(motion.depth(), 3, "the deepest frame drives the depth");

        // And nesting it inherits both, so a motion cannot escape an ancestor's
        // accounting either.
        let wrapped = ViewNode::Column(vec![motion]);
        assert_eq!(wrapped.node_count(), 6);
        assert_eq!(wrapped.depth(), 4);
    }

    /// A line's runs are ordinary children, so nothing about it lets a plugin
    /// carry content past the tree budget.
    #[test]
    fn a_lines_runs_count_toward_the_tree_bounds() {
        let line = ViewNode::Line(vec![
            ViewNode::text("dot"),
            ViewNode::text("name"),
            ViewNode::Line(vec![ViewNode::text("nested")]),
        ]);
        // 1 line + 2 runs + (1 nested line + its run).
        assert_eq!(line.node_count(), 5);
        assert_eq!(line.depth(), 3);
    }

    #[test]
    fn text_and_nested_lines_are_inlineable() {
        assert!(ViewNode::text("run").is_inlineable());
        assert!(ViewNode::Line(vec![]).is_inlineable());
        assert!(ViewNode::Line(vec![ViewNode::text("a"), ViewNode::text("b")]).is_inlineable());
    }

    #[test]
    fn nodes_without_an_intrinsic_width_are_not_inlineable() {
        // Each of these would have to be measured against the area it is given
        // rather than against its own content, which is what a line cannot do.
        for node in [
            ViewNode::Row(vec![]),
            ViewNode::Column(vec![]),
            ViewNode::List(vec![]),
            ViewNode::Divider,
            ViewNode::Spacer { lines: 1 },
        ] {
            assert!(!node.is_inlineable(), "{}", node.kind_name());
            assert_eq!(node.first_non_inlineable(), Some(node.kind_name()));
        }
    }

    #[test]
    fn a_motion_is_inlineable_exactly_when_its_frames_are() {
        let text_frames = ViewNode::Motion {
            key: "k".to_string(),
            keyed_by_id: true,
            motion: super::super::motion::Motion::cycle(
                vec![ViewNode::text("."), ViewNode::text("..")],
                8,
                true,
            ),
        };
        assert!(text_frames.is_inlineable());

        // The rule has to hold through a frame, or a plugin would reach a line
        // with a column by wrapping it in an animation.
        let column_frame = ViewNode::Motion {
            key: "k".to_string(),
            keyed_by_id: true,
            motion: super::super::motion::Motion::cycle(
                vec![
                    ViewNode::text("."),
                    ViewNode::Column(vec![ViewNode::text("..")]),
                ],
                8,
                true,
            ),
        };
        assert!(!column_frame.is_inlineable());
        assert_eq!(
            column_frame.first_non_inlineable(),
            Some("column"),
            "the error must name the column, not the motion carrying it"
        );
    }

    #[test]
    fn every_kind_names_itself() {
        assert_eq!(ViewNode::text("x").kind_name(), "text");
        assert_eq!(ViewNode::Line(vec![]).kind_name(), "line");
        assert_eq!(ViewNode::Row(vec![]).kind_name(), "row");
        assert_eq!(ViewNode::Column(vec![]).kind_name(), "column");
        assert_eq!(ViewNode::List(vec![]).kind_name(), "list");
        assert_eq!(ViewNode::Divider.kind_name(), "divider");
        assert_eq!(ViewNode::Spacer { lines: 1 }.kind_name(), "spacer");
    }

    #[test]
    fn nesting_is_preserved() {
        let tree = ViewNode::Column(vec![
            ViewNode::Row(vec![ViewNode::text("a"), ViewNode::text("b")]),
            ViewNode::text("c"),
        ]);
        assert_eq!(tree.children().len(), 2);
        assert_eq!(tree.children()[0].children().len(), 2);
        assert_eq!(tree.depth(), 3);
        assert_eq!(tree.node_count(), 5);
    }

    #[test]
    fn an_empty_container_is_valid() {
        let tree = ViewNode::Column(vec![]);
        assert_eq!(tree.node_count(), 1);
        assert_eq!(tree.depth(), 1);
    }

    #[test]
    fn leaves_are_depth_one() {
        assert_eq!(ViewNode::Divider.depth(), 1);
        assert_eq!(ViewNode::Spacer { lines: 2 }.depth(), 1);
        assert_eq!(ViewNode::text("x").depth(), 1);
    }

    #[test]
    fn style_tokens_round_trip() {
        for t in StyleToken::all() {
            assert_eq!(StyleToken::parse(t.as_str()), Some(*t));
        }
    }

    #[test]
    fn an_unknown_style_token_does_not_parse() {
        assert_eq!(StyleToken::parse("chartreuse"), None);
        assert_eq!(StyleToken::parse(""), None);
    }

    #[test]
    fn a_node_without_a_token_defaults() {
        let node = ViewNode::text("plain");
        match node {
            ViewNode::Text { style, .. } => {
                assert_eq!(style.token, None);
                assert!(!style.bold);
            }
            other => panic!("expected text, got {other:?}"),
        }
    }

    #[test]
    fn escape_sequences_are_stripped() {
        let node = ViewNode::text("\x1b[31mred\x1b[0m and \x07bell");
        match node {
            // The escape *introducer* is a control char and is dropped; the
            // remaining "[31m" is ordinary text and cannot recolour anything.
            ViewNode::Text { content, .. } => {
                assert!(!content.contains('\x1b'), "{content:?}");
                assert!(!content.contains('\x07'), "{content:?}");
                assert!(content.contains("red"));
            }
            other => panic!("expected text, got {other:?}"),
        }
    }

    #[test]
    fn newlines_are_dropped_rather_than_breaking_layout() {
        let out = sanitize_text("one\ntwo");
        assert_eq!(out, "onetwo");
    }

    #[test]
    fn tabs_become_spaces() {
        assert_eq!(sanitize_text("a\tb"), "a    b");
    }

    #[test]
    fn overlong_text_is_truncated_not_rejected() {
        let long = "x".repeat(MAX_TEXT_LEN * 2);
        assert_eq!(sanitize_text(&long).chars().count(), MAX_TEXT_LEN);
    }

    #[test]
    fn truncation_lands_on_a_character_boundary() {
        // Every char is 4 bytes, so a byte-index cut would split one and the
        // result would not be valid UTF-8.
        let long = "😀".repeat(MAX_TEXT_LEN + 10);
        let out = sanitize_text(&long);
        assert_eq!(out.chars().count(), MAX_TEXT_LEN);
        assert!(out.chars().all(|c| c == '😀'));
    }

    #[test]
    fn node_and_depth_counts_respect_the_bounds_constants() {
        // A tree built right at the depth limit is measurable without
        // overflowing — the conversion layer rejects anything past it.
        let mut node = ViewNode::text("leaf");
        for _ in 1..MAX_DEPTH {
            node = ViewNode::Column(vec![node]);
        }
        assert_eq!(node.depth(), MAX_DEPTH);
        assert_eq!(node.node_count(), MAX_DEPTH);
    }

    #[test]
    fn equality_is_structural_so_unchanged_trees_compare_equal() {
        let a = ViewNode::List(vec![ViewNode::text("one"), ViewNode::Divider]);
        let b = ViewNode::List(vec![ViewNode::text("one"), ViewNode::Divider]);
        let c = ViewNode::List(vec![ViewNode::text("two"), ViewNode::Divider]);
        assert_eq!(a, b, "an unchanged tree must not look changed");
        assert_ne!(a, c);
    }
}
