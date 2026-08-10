//! The workspace tree: how the screen is divided into regions.
//!
//! Pane geometry is a **tree of splits** rather than a fixed set of named
//! rects. A branch splits its rect along one axis and carries its children in
//! draw order; a leaf names one region. Nothing here computes a rectangle —
//! this is the shape, and `ui::layout` solves it.
//!
//! It lives in `session` (pure data, no crate-internal references) because the
//! tree is destined to be loadable from a config file, and config loaders live
//! in `agent`, which may reference `session` but never `ui`.

/// The axis a branch divides its rect along.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// Children sit side by side, sharing the width.
    Horizontal,
    /// Children stack, sharing the height.
    Vertical,
}

/// How a child claims its share of the parent's extent along the parent's axis.
///
/// The three variants are exhaustive of what the layout needs: a fixed band, a
/// proportional column, and "take the rest". A region that must never vanish
/// states its floor on `Fill` rather than relying on the order children are
/// solved in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sizing {
    /// Exactly this many cells. Zero is legal and means an occupied-but-empty
    /// band: the child resolves to zero extent and its siblings shift as if it
    /// were absent.
    Cells(u16),
    /// This percentage of the parent's extent.
    Percent(u16),
    /// Whatever is left after the fixed and proportional siblings, but never
    /// less than `min`.
    Fill { min: u16 },
}

/// A region the layout can place. Each id is placed at most once per tree, so a
/// consumer looking one up can never get two answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RegionId {
    /// Kernel chrome along the top edge.
    Header,
    /// Kernel chrome along the bottom edge.
    Footer,
    /// The session list.
    SessionList,
    /// The automations pane, beneath the session list.
    Automations,
    /// The info panel, left of the center.
    Info,
    /// The central pane — agent terminal, shell, or code review.
    Center,
    /// The tasks panel.
    Tasks,
    /// The file viewer (or a review's changed-files list).
    FileViewer,
    /// The full-width global-search strip.
    GlobalSearch,
    /// The transient full-width status/error band.
    StatusMessage,
    /// The nth visible plugin-contributed pane, in publication order.
    ///
    /// Indexed rather than singular because a plugin may publish several panes
    /// and the layout has to seat all of them — a single slot is what made
    /// migrating a native panel to a plugin mean evicting whatever was there.
    Plugin(usize),
}

/// What a node is: a leaf naming a region, or a branch splitting its rect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Region {
    /// A leaf: this node's whole rect belongs to one region.
    Pane(RegionId),
    /// A branch: the rect is divided along `axis` between `children`, in order.
    Split { axis: Axis, children: Vec<Node> },
}

/// One node of the workspace tree: how it is sized within its parent, and what
/// it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    /// The node's share of its parent's extent. Ignored at the root, which
    /// takes the whole area.
    pub sizing: Sizing,
    pub region: Region,
}

impl Node {
    /// A leaf placing `id`.
    pub fn pane(sizing: Sizing, id: RegionId) -> Self {
        Self {
            sizing,
            region: Region::Pane(id),
        }
    }

    /// A branch dividing its rect along `axis`.
    pub fn split(sizing: Sizing, axis: Axis, children: Vec<Node>) -> Self {
        Self {
            sizing,
            region: Region::Split { axis, children },
        }
    }

    /// Every region this subtree places, in draw order.
    ///
    /// Used to assert the one-region-one-rect invariant in tests without
    /// solving; a solver is not needed to know what a tree names.
    pub fn region_ids(&self) -> Vec<RegionId> {
        let mut out = Vec::new();
        self.collect_region_ids(&mut out);
        out
    }

    fn collect_region_ids(&self, out: &mut Vec<RegionId>) {
        match &self.region {
            Region::Pane(id) => out.push(*id),
            Region::Split { children, .. } => {
                for child in children {
                    child.collect_region_ids(out);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nested() -> Node {
        Node::split(
            Sizing::Fill { min: 0 },
            Axis::Vertical,
            vec![
                Node::pane(Sizing::Cells(1), RegionId::Header),
                Node::split(
                    Sizing::Fill { min: 1 },
                    Axis::Horizontal,
                    vec![
                        Node::pane(Sizing::Percent(18), RegionId::SessionList),
                        Node::pane(Sizing::Fill { min: 0 }, RegionId::Center),
                        Node::pane(Sizing::Percent(20), RegionId::Plugin(0)),
                    ],
                ),
                Node::pane(Sizing::Cells(1), RegionId::Footer),
            ],
        )
    }

    #[test]
    fn workspace_tree_leaf_names_one_region() {
        let leaf = Node::pane(Sizing::Cells(3), RegionId::Tasks);
        assert_eq!(leaf.region_ids(), vec![RegionId::Tasks]);
        assert_eq!(leaf.sizing, Sizing::Cells(3));
    }

    #[test]
    fn workspace_tree_lists_regions_in_draw_order() {
        assert_eq!(
            nested().region_ids(),
            vec![
                RegionId::Header,
                RegionId::SessionList,
                RegionId::Center,
                RegionId::Plugin(0),
                RegionId::Footer,
            ]
        );
    }

    #[test]
    fn workspace_tree_names_each_region_at_most_once() {
        let ids = nested().region_ids();
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(ids.len(), unique.len(), "a region is placed twice: {ids:?}");
    }

    #[test]
    fn workspace_tree_plugin_regions_are_distinct_per_index() {
        assert_ne!(RegionId::Plugin(0), RegionId::Plugin(1));
    }
}
