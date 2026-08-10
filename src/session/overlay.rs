//! Anchored overlays: a rect placed against another rect.
//!
//! The workspace tree ([`crate::session::workspace_tree`]) divides space so that
//! nothing overlaps. Some furniture genuinely wants to overlap — a completion
//! dropdown, a context menu, a comment box floating at the line it is about —
//! and the answer is not a slot per case but an **anchor**: state which side of
//! a target rect you want to sit on, and let the kernel resolve it against the
//! space actually available.
//!
//! Resolution is one pure function, [`Overlay::place`], in four steps: clamp the
//! declared extents to the clip, take the requested side if it fits, take the
//! opposite side if flipping is allowed and it fits, else dock flush against the
//! clip's edge in the requested direction. The last step is also what an
//! **absent** target resolves to — the honest reading of "the line you are
//! pointing at has scrolled out of view".
//!
//! It lives in `session` (pure data + pure geometry, no crate-internal
//! references) for the same reason the workspace tree does: an anchor is
//! destined to be declarable in config, and config loaders live in `agent`,
//! which may reference `session` but never `ui`.

use ratatui::layout::Rect;

/// Which side of the target the overlay sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Above the target: the overlay's bottom edge is the target's top edge.
    Above,
    /// Below the target: the overlay's top edge is the target's bottom edge.
    Below,
    /// Left of the target: the overlay's right edge is the target's left edge.
    Left,
    /// Right of the target: the overlay's left edge is the target's right edge.
    Right,
}

impl Side {
    /// Whether this side anchors along the vertical axis.
    fn is_vertical(self) -> bool {
        matches!(self, Side::Above | Side::Below)
    }

    /// Whether the overlay sits *after* the target along the anchored axis.
    fn is_trailing(self) -> bool {
        matches!(self, Side::Below | Side::Right)
    }

    /// The side taken when the requested one has no room.
    fn opposite(self) -> Self {
        match self {
            Side::Above => Side::Below,
            Side::Below => Side::Above,
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }
}

/// Where a fixed-width overlay sits **across** the anchored axis, relative to
/// the target. Ignored by [`CrossExtent::Stretch`], which spans the clip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    /// Share the target's leading edge (its left, or its top).
    Start,
    /// Centre on the target.
    Center,
    /// Share the target's trailing edge (its right, or its bottom).
    End,
}

/// The overlay's extent **across** the anchored axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossExtent {
    /// Exactly this many cells, positioned by [`Align`].
    Cells(u16),
    /// Span the clip, inset this many cells at each end — the shape of a banner
    /// that belongs to the whole pane rather than to a column of it.
    Stretch { inset: u16 },
}

/// An overlay's declaration: how big it is and which side of its target it wants.
///
/// The target itself is not named here. Resolving an id to a rect belongs to
/// whoever owns the id space — and on this branch a pane's contents are Rust
/// render functions, so callers pass the rect they already have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Overlay {
    /// The side of the target to sit on.
    pub side: Side,
    /// Whether the opposite side may be used when `side` has no room.
    pub flip: bool,
    /// Extent along `side`'s axis.
    pub main: u16,
    /// Extent across `side`'s axis.
    pub cross: CrossExtent,
    /// How a fixed `cross` extent aligns against the target.
    pub align: Align,
}

/// One axis of a rect, as `(start, len)`.
type Span = (i32, i32);

impl Overlay {
    /// Resolve this overlay against `target`, inside `clip`.
    ///
    /// The result is always contained in `clip`: extents are clamped to it
    /// first, so an overlay bigger than its pane is shrunk rather than allowed
    /// to paint over a neighbour. `target` is `None` when the thing being
    /// anchored to is not on screen; the overlay then docks, exactly as it does
    /// when neither side has room.
    pub fn place(&self, target: Option<Rect>, clip: Rect) -> Rect {
        let (clip_main, clip_cross) = self.split_axes(clip);
        let target_axes = target.map(|t| self.split_axes(t));

        let main_len = i32::from(self.main).min(clip_main.1);
        let main_start = self.main_start(target_axes.map(|(m, _)| m), clip_main, main_len);
        let (cross_start, cross_len) =
            self.cross_span(target_axes.map(|(_, c)| c), clip_cross, main_len);

        let main = (main_start, main_len);
        let cross = (cross_start, cross_len);
        let (x, width) = if self.side.is_vertical() { cross } else { main };
        let (y, height) = if self.side.is_vertical() { main } else { cross };
        Rect::new(x as u16, y as u16, width as u16, height as u16)
    }

    /// `rect`'s spans as `(along the anchored axis, across it)`.
    fn split_axes(&self, rect: Rect) -> (Span, Span) {
        let horizontal = (i32::from(rect.x), i32::from(rect.width));
        let vertical = (i32::from(rect.y), i32::from(rect.height));
        if self.side.is_vertical() {
            (vertical, horizontal)
        } else {
            (horizontal, vertical)
        }
    }

    /// The overlay's start along the anchored axis: requested side, else the
    /// flipped side, else docked against the clip.
    fn main_start(&self, target: Option<Span>, clip: Span, len: i32) -> i32 {
        let (clip_start, clip_len) = clip;
        let clip_end = clip_start + clip_len;
        let docked = if self.side.is_trailing() {
            // `len <= clip_len`, so docking can never start before the clip.
            clip_end - len
        } else {
            clip_start
        };
        let Some((t_start, t_len)) = target else {
            return docked;
        };
        let anchored = |side: Side| {
            if side.is_trailing() {
                t_start + t_len
            } else {
                t_start - len
            }
        };
        let fits = |start: i32| start >= clip_start && start + len <= clip_end;

        let preferred = anchored(self.side);
        if fits(preferred) {
            return preferred;
        }
        if self.flip {
            let flipped = anchored(self.side.opposite());
            if fits(flipped) {
                return flipped;
            }
        }
        docked
    }

    /// The overlay's span across the anchored axis.
    ///
    /// `main_len` gates a stretch: an overlay the clip squeezed to nothing along
    /// its own axis has no cells to paint across it either, so it collapses
    /// rather than reporting a wide, zero-tall band.
    fn cross_span(&self, target: Option<Span>, clip: Span, main_len: i32) -> Span {
        let (clip_start, clip_len) = clip;
        match self.cross {
            CrossExtent::Stretch { inset } => {
                if main_len == 0 {
                    return (clip_start, 0);
                }
                let inset = i32::from(inset);
                // At least one cell, so a pane too narrow for the insets still
                // shows the overlay instead of a zero-width rect.
                let len = (clip_len - inset * 2).max(1).min(clip_len);
                ((clip_start + inset).min(clip_start + clip_len - len), len)
            }
            CrossExtent::Cells(cells) => {
                let len = i32::from(cells).min(clip_len);
                let start = match target {
                    Some((t_start, t_len)) => match self.align {
                        Align::Start => t_start,
                        Align::Center => t_start + (t_len - len) / 2,
                        Align::End => t_start + t_len - len,
                    },
                    // Nothing to align against; the clip's leading edge is the
                    // only defensible answer.
                    None => clip_start,
                };
                (start.clamp(clip_start, clip_start + clip_len - len), len)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 40x20 pane at a non-zero origin, so a bug that ignores the clip's
    /// origin cannot pass by arithmetic coincidence.
    fn clip() -> Rect {
        Rect::new(5, 10, 40, 20)
    }

    /// A one-row, full-width target — the shape of a diff row's hitbox.
    fn row(y: u16) -> Rect {
        Rect::new(5, y, 40, 1)
    }

    fn banner(side: Side, flip: bool, main: u16) -> Overlay {
        Overlay {
            side,
            flip,
            main,
            cross: CrossExtent::Stretch { inset: 1 },
            align: Align::Start,
        }
    }

    #[test]
    fn overlay_sits_flush_against_the_requested_side() {
        let placed = banner(Side::Below, true, 4).place(Some(row(12)), clip());
        assert_eq!(placed.y, 13, "top edge is the target's bottom edge");
        assert_eq!(placed.height, 4);
    }

    #[test]
    fn overlay_places_each_side_adjacent_to_the_target() {
        let target = Rect::new(20, 15, 6, 3);
        let boxed = |side| Overlay {
            side,
            flip: false,
            main: 4,
            cross: CrossExtent::Cells(6),
            align: Align::Start,
        };
        let below = boxed(Side::Below).place(Some(target), clip());
        assert_eq!(below.y, 18, "below: target bottom edge");
        let above = boxed(Side::Above).place(Some(target), clip());
        assert_eq!(above.y + above.height, 15, "above: target top edge");
        let right = boxed(Side::Right).place(Some(target), clip());
        assert_eq!(right.x, 26, "right: target right edge");
        let left = boxed(Side::Left).place(Some(target), clip());
        assert_eq!(left.x + left.width, 20, "left: target left edge");
    }

    #[test]
    fn overlay_flips_when_the_requested_side_has_no_room() {
        // Row 28 leaves one row below inside a clip ending at 30, but 18 above.
        let placed = banner(Side::Below, true, 4).place(Some(row(28)), clip());
        assert_eq!(placed.y + placed.height, 28, "flipped above the target row");
    }

    #[test]
    fn overlay_prefers_the_requested_side_when_it_fits() {
        // Room on both sides: the request wins.
        let placed = banner(Side::Below, true, 4).place(Some(row(20)), clip());
        assert_eq!(placed.y, 21);
    }

    #[test]
    fn overlay_without_flip_stays_on_its_side() {
        let placed = banner(Side::Below, false, 4).place(Some(row(28)), clip());
        assert_eq!(placed.y + placed.height, 30, "docked, not flipped above");
    }

    #[test]
    fn overlay_docks_when_neither_side_has_room() {
        // A 12-row overlay in a 20-row clip: row 15 leaves 14 below and 5 above,
        // so neither side can hold it and it docks in the requested direction.
        let placed = banner(Side::Below, true, 12).place(Some(row(19)), clip());
        assert_eq!(placed.y + placed.height, 30, "flush with the clip's bottom");
        let up = banner(Side::Above, true, 12).place(Some(row(19)), clip());
        assert_eq!(up.y, 10, "flush with the clip's top");
    }

    #[test]
    fn overlay_with_no_target_docks_like_one_with_no_room() {
        let none = banner(Side::Below, true, 5).place(None, clip());
        let crowded = banner(Side::Below, true, 5).place(Some(row(27)), clip());
        // Row 27 leaves 2 rows below and 17 above, so `crowded` flips; the
        // docked placement is the one an absent target must match.
        let docked = banner(Side::Below, false, 5).place(Some(row(27)), clip());
        assert_eq!(none, docked);
        assert_ne!(none, crowded, "flipping is still preferred to docking");
    }

    #[test]
    fn overlay_is_contained_in_its_clip_for_every_target_position() {
        let clip = clip();
        for main in [1_u16, 3, 6, 20, 25] {
            for flip in [true, false] {
                for side in [Side::Above, Side::Below, Side::Left, Side::Right] {
                    // Sweep the target across the clip and past both edges.
                    for y in 0..40_u16 {
                        let placed = banner(side, flip, main).place(Some(row(y)), clip);
                        assert!(
                            placed.x >= clip.x
                                && placed.y >= clip.y
                                && placed.x + placed.width <= clip.x + clip.width
                                && placed.y + placed.height <= clip.y + clip.height,
                            "{placed:?} escapes {clip:?} (side {side:?}, flip {flip}, main {main})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn overlay_larger_than_its_clip_is_shrunk_to_fit() {
        let tiny = Rect::new(0, 0, 10, 2);
        let placed = banner(Side::Below, true, 6).place(Some(row(0)), tiny);
        assert_eq!(
            placed.height, 2,
            "shrunk to the clip rather than overflowing"
        );
        assert_eq!(placed.y, 0);
    }

    #[test]
    fn overlay_stretch_spans_the_clip_inset_at_both_ends() {
        let placed = banner(Side::Below, true, 4).place(Some(row(12)), clip());
        assert_eq!(placed.x, 6, "one cell inside the clip's left edge");
        assert_eq!(placed.width, 38, "the clip's width less both insets");
    }

    #[test]
    fn overlay_stretch_keeps_one_cell_in_a_clip_narrower_than_its_insets() {
        let narrow = Rect::new(3, 0, 1, 6);
        let placed = banner(Side::Below, true, 3).place(Some(Rect::new(3, 0, 1, 1)), narrow);
        assert_eq!((placed.x, placed.width), (3, 1));
    }

    #[test]
    fn overlay_alignment_anchors_to_each_target_edge() {
        let target = Rect::new(20, 12, 9, 1);
        let aligned = |align| {
            Overlay {
                side: Side::Below,
                flip: true,
                main: 3,
                cross: CrossExtent::Cells(3),
                align,
            }
            .place(Some(target), clip())
        };
        assert_eq!(aligned(Align::Start).x, 20, "shares the target's left edge");
        assert_eq!(
            aligned(Align::End).x + 3,
            29,
            "shares the target's right edge"
        );
        assert_eq!(aligned(Align::Center).x, 23, "centred on the target");
    }

    #[test]
    fn overlay_alignment_is_clamped_into_the_clip() {
        // The target's right edge is the clip's, so an End-aligned overlay would
        // start inside but a Start-aligned one would run off.
        let target = Rect::new(41, 12, 4, 1);
        let placed = Overlay {
            side: Side::Below,
            flip: true,
            main: 3,
            cross: CrossExtent::Cells(8),
            align: Align::Start,
        }
        .place(Some(target), clip());
        assert_eq!(placed.width, 8, "the width is kept");
        assert_eq!(
            placed.x + placed.width,
            45,
            "clamped to the clip's right edge"
        );
    }

    #[test]
    fn overlay_collapses_when_the_clip_has_no_room_along_its_axis() {
        let flat = Rect::new(0, 0, 30, 0);
        let placed = banner(Side::Below, true, 4).place(None, flat);
        assert_eq!((placed.width, placed.height), (0, 0));
    }
}
