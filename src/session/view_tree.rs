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
///
/// The set covers every *distinct* palette role thurbox's own panes draw with,
/// which is what lets a pane be rendered through this tree without reaching
/// past it for a colour. Two overlaps are deliberate: [`StyleToken::Warning`]
/// and [`StyleToken::StatusWorking`] resolve alike today, as do
/// [`StyleToken::Success`] and [`StyleToken::StatusIdle`]. The first pair of
/// each is a token a plugin picks for *its own* meaning; the second is the
/// token for *the kernel's* session status, where the token is the meaning and
/// an approximation is wrong rather than merely different. Collapsing them
/// would leave a pane drawing status indicators picking `warning` for working
/// and `status_blocked` for blocked.
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
    /// Text one step below primary but above muted.
    Secondary,
    /// The name of an agent or other actor.
    Role,
    /// A git branch or repository name.
    Branch,
    /// The accent's brighter partner — a heading inside an accented region, or
    /// a type name in highlighted code.
    AccentBright,
    /// An addition — inserted lines, a positive delta.
    Added,
    /// The colour a pane's own borders and rules are drawn in.
    Border,
    /// A session actively running.
    StatusWorking,
    /// A session waiting on the user.
    StatusBlocked,
    /// A session whose turn just finished.
    StatusDone,
    /// A session at rest.
    StatusIdle,
    /// A session that failed.
    StatusError,
    /// A session whose host cannot be reached.
    StatusUnreachable,
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
            StyleToken::Secondary => "secondary",
            StyleToken::Role => "role",
            StyleToken::Branch => "branch",
            StyleToken::AccentBright => "accent_bright",
            StyleToken::Added => "added",
            StyleToken::Border => "border",
            StyleToken::StatusWorking => "status_working",
            StyleToken::StatusBlocked => "status_blocked",
            StyleToken::StatusDone => "status_done",
            StyleToken::StatusIdle => "status_idle",
            StyleToken::StatusError => "status_error",
            StyleToken::StatusUnreachable => "status_unreachable",
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
            StyleToken::Secondary,
            StyleToken::Role,
            StyleToken::Branch,
            StyleToken::AccentBright,
            StyleToken::Added,
            StyleToken::Border,
            StyleToken::StatusWorking,
            StyleToken::StatusBlocked,
            StyleToken::StatusDone,
            StyleToken::StatusIdle,
            StyleToken::StatusError,
            StyleToken::StatusUnreachable,
        ]
    }

    /// The token that draws `status`, so a pane never maps statuses to colour
    /// roles by hand and two panes cannot disagree about which colour a state
    /// gets.
    pub fn for_status(status: super::SessionStatus) -> StyleToken {
        match status {
            super::SessionStatus::Working => StyleToken::StatusWorking,
            super::SessionStatus::Blocked => StyleToken::StatusBlocked,
            super::SessionStatus::Done => StyleToken::StatusDone,
            super::SessionStatus::Idle => StyleToken::StatusIdle,
            super::SessionStatus::Error => StyleToken::StatusError,
            super::SessionStatus::Unreachable => StyleToken::StatusUnreachable,
        }
    }

    /// Parse a wire name, or `None` if the host does not define it.
    pub fn parse(s: &str) -> Option<StyleToken> {
        StyleToken::all().iter().copied().find(|t| t.as_str() == s)
    }
}

/// Which side of a change a diff row is on.
///
/// Two members and no third: the vocabulary is the pair of palette fields the
/// theme defines for it (`diff_added_bg`, `diff_removed_bg`), and a context row
/// carries no tint at all rather than a "none" member — an absent role and a
/// role meaning "nothing" would be two spellings of one state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiffTint {
    /// An inserted line.
    Added,
    /// A deleted line.
    Removed,
}

impl DiffTint {
    /// The wire name a plugin uses.
    pub fn as_str(self) -> &'static str {
        match self {
            DiffTint::Added => "added",
            DiffTint::Removed => "removed",
        }
    }

    /// Every tint the host defines, for an error that has to list them.
    pub fn all() -> &'static [DiffTint] {
        &[DiffTint::Added, DiffTint::Removed]
    }

    /// Parse a wire name, or `None` if the host does not define it.
    pub fn parse(s: &str) -> Option<DiffTint> {
        DiffTint::all().iter().copied().find(|t| t.as_str() == s)
    }
}

/// Text styling flags a node may carry alongside its colour token.
///
/// The three emphases are terminal attributes, never colours: each is applied
/// *over* whatever [`StyleToken`] the run resolves to, so a theme still chooses
/// every colour in a pane. They exist because a selectable list row needs three
/// appearances a colour token alone cannot express — the selected row, a row a
/// running search filtered out ([`TextStyle::dim`]), and the characters that
/// search matched ([`TextStyle::underline`]). Without them a list with a search
/// in it cannot be described by this catalog at all, which is most of the panes
/// thurbox has.
///
/// [`TextStyle::selected`] is the one field that is **not** an emphasis, and it
/// is documented on itself.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct TextStyle {
    /// Colour role; `None` renders in the theme's default foreground.
    pub token: Option<StyleToken>,
    /// Render bold.
    pub bold: bool,
    /// Render dimmed.
    pub dim: bool,
    /// Render underlined.
    pub underline: bool,
    /// The run belongs to the row the user's cursor is on.
    ///
    /// A **role**, not a colour and not an attribute: the host resolves it to
    /// the theme's selection foreground on its selection background, so a
    /// plugin naming it still names none of the two colours involved. It
    /// exists because thurbox's file viewer draws its cursor's row as a
    /// background, and nothing in this catalog could name one.
    ///
    /// It *replaces* the colour [`TextStyle::token`] would have resolved to,
    /// where the three emphases layer over it — a selection is a whole
    /// appearance rather than an attribute applied to one. It composes with
    /// the emphases, so a selected run can also be bold.
    ///
    /// Deliberately separate from [`ViewNode::List`]'s selected row, which
    /// says *which* row the cursor is on: thurbox's two list panes disagree
    /// about what a selected row looks like (the tasks pane draws it accent and
    /// bold, the file viewer in the selection pair), so an appearance inferred
    /// from the list's cursor would make one of them unreproducible.
    pub selected: bool,
    /// The run's row is a diff insertion or deletion.
    ///
    /// A **role** like [`TextStyle::selected`], and for the same reason: the
    /// host resolves it to the theme's added-row or removed-row background, so
    /// the plugin names which side of a change the row is on and never a
    /// colour. Unlike a selection it leaves the foreground to
    /// [`TextStyle::token`], because a diff body's colours are the pane's
    /// (syntax highlighting) while the tint is the only thing carrying add
    /// versus remove — the gutter's sign is one character wide.
    ///
    /// [`TextStyle::selected`] wins over it. The cursor's row is one appearance
    /// whatever it contains, and two backgrounds on one row is not a state the
    /// theme defines — the rule `ui::code_review`'s own renderer already
    /// applies.
    pub tint: Option<DiffTint>,
}

/// A gauge's fill, as a percentage.
///
/// Wrapped rather than stored as a bare `f32` because [`ViewNode`] derives
/// `Eq` and `Hash`, and that is load-bearing: an identical re-push must be
/// recognised as unchanged so a motion elsewhere in the tree keeps its epoch.
/// A bare float would take `Eq`/`Hash` off the whole enum.
///
/// Comparison is on the bit pattern, which is the right question here — two
/// gauges are "the same node" exactly when the host would draw them
/// identically.
///
/// Deliberately **not** normalised at construction: the renderer clamps at the
/// moment of drawing, so a value out of range behaves as it always did rather
/// than being rewritten on the way in.
#[derive(Debug, Clone, Copy)]
pub struct Percent(f32);

impl Percent {
    /// Carry `value` as a gauge percentage.
    pub fn new(value: f32) -> Percent {
        Percent(value)
    }

    /// The value as given.
    pub fn get(self) -> f32 {
        self.0
    }
}

impl PartialEq for Percent {
    fn eq(&self, other: &Percent) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for Percent {}

impl std::hash::Hash for Percent {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
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
    /// Inline runs laid out left to right and **soft-wrapped** onto further
    /// rows when they exceed the available width.
    ///
    /// The counterpart to [`ViewNode::Line`], which clips at one row. Both
    /// exist because a pane needs both rules: a row of fixed-width fields must
    /// not push its neighbours down, while agent-supplied prose of unbounded
    /// length must stay readable. A paragraph's height is therefore a function
    /// of the width it is given — the one node in the catalog of which that is
    /// true.
    ///
    /// Admits exactly the children a line does; see [`ViewNode::is_inlineable`].
    Paragraph(Vec<ViewNode>),
    /// Children laid out top to bottom.
    Column(Vec<ViewNode>),
    /// Children laid out top to bottom, one per line — a column that reads as
    /// a list to a user, and the shape most panes actually want.
    List {
        /// One child per row.
        children: Vec<ViewNode>,
        /// Which child the user's cursor is on, if any.
        ///
        /// A **zero-based** index here; the plugin-facing form is one-based,
        /// converted once at conversion, because every other array a plugin
        /// touches is Lua's one-based kind.
        ///
        /// When it is present and the list has more children than the rows it
        /// was given, the *kernel* chooses which slice to draw and keeps this
        /// child visible — the same trade [`ViewNode::Gauge`] made for width,
        /// applied to height. The alternative is reporting the resolved rect
        /// back into a plugin, which would make rendering height-dependent (a
        /// resize would have to re-enter a VM before the frame that needed it)
        /// and would let a plugin that mis-windowed produce a broken pane
        /// rather than a refused node.
        ///
        /// The window is chosen by child *index*, so a selected list is a list
        /// of rows; a child taller than one row is clipped by the bottom as in
        /// any list.
        selected: Option<usize>,
        /// Whether the list wants a scroll track — the rightmost column of its
        /// area, carrying a thumb, when it has more children than rows.
        ///
        /// The same trade `selected` makes for height, applied to the one column
        /// a track occupies: the list declares *that* it scrolls and the kernel
        /// decides where that is shown. A plugin is told neither dimension, so it
        /// could not place a track itself.
        ///
        /// Deliberately **not** inferred from `selected`. Three of thurbox's own
        /// panes draw selectable lists that overflow without a scrollbar
        /// (`ui::tasks_panel`, `ui::automations_panel`, `ui::project_list`), so
        /// inferring one would put a track into panes that deliberately have
        /// none — and would move their frames.
        scrollbar: bool,
    },
    /// A horizontal rule filling the available width.
    Divider,
    /// A labelled progress bar, two rows tall, whose geometry the **kernel**
    /// resolves from the area it is given.
    ///
    /// The node exists so that drawing one needs no access to a resolved pane
    /// width. Reporting the rect back instead would make rendering
    /// width-dependent — a resize would have to re-enter a plugin's VM before
    /// the frame that needed it — and a plugin that mis-measured would produce a
    /// broken pane rather than a refused node.
    Gauge {
        /// Drawn at the left of the header row.
        label: String,
        /// How full the bar is, clamped to 0–100 when drawn.
        percent: Percent,
        /// Flush-right header text; `None` draws the rounded percentage.
        suffix: Option<String>,
    },
    /// One repeated glyph across whatever width is left on its line.
    ///
    /// An inline run whose width is the *residue* of the line it sits on, which
    /// is why the kernel resolves it: a plugin is never told the width it got,
    /// and a background that stops where its text stops is not a diff row's
    /// tint — the tint reaches the pane's right edge. Same trade as
    /// [`ViewNode::Gauge`] for a bar and [`ViewNode::List`] for a scroll window,
    /// applied to a line's leftover columns.
    ///
    /// It carries a full [`TextStyle`], so a fill can be tinted (which is what
    /// makes a diff row's background reach the edge) or drawn in a colour role
    /// (which is what a rule trailing a heading is).
    Fill {
        /// The glyph to repeat. One character, sanitized at construction.
        glyph: char,
        /// How to draw it.
        style: TextStyle,
    },
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
            ViewNode::Row(c) | ViewNode::Line(c) | ViewNode::Paragraph(c) | ViewNode::Column(c) => {
                c
            }
            ViewNode::List { children, .. } => children,
            // A motion's frames are its children: that is what makes them
            // count against the tree budget rather than escaping it.
            ViewNode::Motion { motion, .. } => motion.frames(),
            ViewNode::Text { .. }
            | ViewNode::Divider
            | ViewNode::Fill { .. }
            | ViewNode::Gauge { .. }
            | ViewNode::Spacer { .. } => &[],
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
            // A fill is inlineable by design: it is the one run whose width the
            // *line* decides, which only means anything on a line.
            ViewNode::Text { .. } | ViewNode::Fill { .. } => None,
            ViewNode::Line(children) => children.iter().find_map(ViewNode::first_non_inlineable),
            ViewNode::Motion { motion, .. } => motion
                .frames()
                .iter()
                .find_map(ViewNode::first_non_inlineable),
            // A paragraph's *children* are inlineable, but a paragraph is not:
            // its height is however many rows the wrap needs, which is the one
            // thing a one-row line cannot hold. A gauge likewise takes its width
            // from its area rather than its content.
            ViewNode::Row(_)
            | ViewNode::Paragraph(_)
            | ViewNode::Column(_)
            | ViewNode::List { .. }
            | ViewNode::Divider
            | ViewNode::Gauge { .. }
            | ViewNode::Spacer { .. } => Some(self.kind_name()),
        }
    }

    /// The node kind's wire name, for an error that has to name it.
    pub fn kind_name(&self) -> &'static str {
        match self {
            ViewNode::Text { .. } => "text",
            ViewNode::Row(_) => "row",
            ViewNode::Line(_) => "line",
            ViewNode::Paragraph(_) => "paragraph",
            ViewNode::Column(_) => "column",
            ViewNode::List { .. } => "list",
            ViewNode::Divider => "divider",
            ViewNode::Fill { .. } => "fill",
            ViewNode::Gauge { .. } => "gauge",
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

    /// Build a text node in a colour role, unbolded — the shape most rows are.
    pub fn token(content: impl Into<String>, token: StyleToken) -> ViewNode {
        ViewNode::styled(
            content,
            TextStyle {
                token: Some(token),
                ..TextStyle::default()
            },
        )
    }

    /// Build a list with no cursor in it — what most lists are.
    ///
    /// A constructor rather than a struct literal at every call site, so that
    /// adding the cursor did not make the common case noisier than it was.
    pub fn list(children: Vec<ViewNode>) -> ViewNode {
        ViewNode::List {
            children,
            selected: None,
            scrollbar: false,
        }
    }

    /// Build a list whose `selected` child (zero-based) the kernel keeps in
    /// view.
    pub fn selectable_list(children: Vec<ViewNode>, selected: Option<usize>) -> ViewNode {
        ViewNode::List {
            children,
            selected,
            scrollbar: false,
        }
    }

    /// Build a list that also asks for a scroll track — the shape a pane which
    /// reserves its rightmost column for a thumb declares.
    ///
    /// Separate from [`ViewNode::selectable_list`] rather than a fourth argument
    /// on it, so the lists that want no track (most of them, including three of
    /// thurbox's own overflowing panes) stay as short to write as they were.
    pub fn scrolling_list(children: Vec<ViewNode>, selected: Option<usize>) -> ViewNode {
        ViewNode::List {
            children,
            selected,
            scrollbar: true,
        }
    }

    /// Build a fill run, sanitizing its glyph the way text is sanitized.
    ///
    /// A control character becomes a space rather than being refused: the glyph
    /// is repeated across a whole row, so an escape sequence here would be the
    /// worst place in the catalog to let one through, and a blank fill is the
    /// same shape the caller asked for.
    pub fn fill(glyph: char, style: TextStyle) -> ViewNode {
        ViewNode::Fill {
            glyph: if glyph.is_control() { ' ' } else { glyph },
            style,
        }
    }

    /// Build a gauge, sanitizing its label and suffix like any other text.
    pub fn gauge(label: impl Into<String>, percent: f32, suffix: Option<String>) -> ViewNode {
        ViewNode::Gauge {
            label: sanitize_text(&label.into()),
            percent: Percent::new(percent),
            suffix: suffix.map(|s| sanitize_text(&s)),
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
            ViewNode::list(vec![]),
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
        assert_eq!(ViewNode::list(vec![]).kind_name(), "list");
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
        let a = ViewNode::list(vec![ViewNode::text("one"), ViewNode::Divider]);
        let b = ViewNode::list(vec![ViewNode::text("one"), ViewNode::Divider]);
        let c = ViewNode::list(vec![ViewNode::text("two"), ViewNode::Divider]);
        assert_eq!(a, b, "an unchanged tree must not look changed");
        assert_ne!(a, c);
    }
}
