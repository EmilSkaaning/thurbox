//! The per-pane overlay layer.
//!
//! A pane declares its anchored rects here as it draws them. The layer exists
//! for the two things the resolver ([`crate::session::overlay::Overlay`]) cannot
//! answer on its own: **order** and **priority**.
//!
//! Order is positional — later declared is drawn on top — so two overlays can
//! never argue about depth the way a `z-index` invites them to. Priority is the
//! consequence: [`OverlayLayer::into_hit_order`] hands the rects back topmost
//! first, and a pane records them as click targets *before* its base-layer rows,
//! so a click that lands on an overlay stops there instead of falling through to
//! whatever it is covering.
//!
//! An overlay is never a focus target. It belongs to the pane that declared it,
//! so exactly one pane still holds focus whether or not overlays are showing.

use ratatui::layout::Rect;

use crate::session::overlay::Overlay;

/// The overlays one pane declared this frame, in declaration order.
#[derive(Debug, Default)]
pub struct OverlayLayer {
    rects: Vec<Rect>,
}

impl OverlayLayer {
    /// Resolve `overlay` against `target` inside `clip`, record it, and return
    /// the rect to draw into.
    ///
    /// `target` is `None` when the anchor's subject is not on screen — a diff
    /// row scrolled out of the viewport, say; the resolver docks in that case
    /// rather than dropping the overlay.
    pub fn place(&mut self, overlay: &Overlay, target: Option<Rect>, clip: Rect) -> Rect {
        let rect = overlay.place(target, clip);
        self.rects.push(rect);
        rect
    }

    /// The recorded rects, **topmost first** — the order hit-testing must
    /// consult them in.
    pub fn into_hit_order(mut self) -> Vec<Rect> {
        self.rects.reverse();
        self.rects
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::overlay::{Align, CrossExtent, Side};

    fn banner(main: u16) -> Overlay {
        Overlay {
            side: Side::Below,
            flip: true,
            main,
            cross: CrossExtent::Stretch { inset: 1 },
            align: Align::Start,
        }
    }

    #[test]
    fn overlay_layer_is_empty_when_nothing_is_anchored() {
        assert!(OverlayLayer::default().into_hit_order().is_empty());
    }

    #[test]
    fn overlay_layer_reports_the_last_declaration_topmost() {
        let clip = Rect::new(0, 0, 20, 20);
        let mut layer = OverlayLayer::default();
        let first = layer.place(&banner(3), Some(Rect::new(0, 2, 20, 1)), clip);
        let second = layer.place(&banner(4), Some(Rect::new(0, 9, 20, 1)), clip);
        assert_eq!(
            layer.into_hit_order(),
            vec![second, first],
            "later declared is on top, so it is hit-tested first"
        );
    }

    #[test]
    fn overlay_layer_places_inside_the_clip() {
        let clip = Rect::new(4, 4, 12, 6);
        let mut layer = OverlayLayer::default();
        let rect = layer.place(&banner(5), Some(Rect::new(4, 8, 12, 1)), clip);
        assert!(
            rect.y >= clip.y && rect.y + rect.height <= clip.y + clip.height,
            "{rect:?} escapes {clip:?}"
        );
    }
}
