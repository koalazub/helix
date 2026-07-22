//! Plugin-driven virtual-line rendering for math layouts.
//!
//! The nothelix concealer needs to render things like `\int_0^1 f(t) dt`
//! with the limits stacked above/below the ∫ glyph rather than shrunk into
//! near-unreadable Unicode super/subscripts. Plugins compute the pre-padded
//! strings (leading whitespace positions the text under the right source
//! column) and stash them on the `Document` via
//! [`helix_view::Document::set_math_lines_above`] /
//! [`helix_view::Document::set_math_lines_below`].
//!
//! This module is the renderer. It implements both
//! [`helix_core::text_annotations::LineAnnotation`] (to reserve virtual rows
//! around the relevant source line) and [`Decoration`] (to actually paint
//! the strings into those reserved rows).
//!
//! Helix's virtual-line machinery reserves rows AFTER a source line — there
//! is no "reserve rows before line N" hook. So "above line N" is emulated
//! by reserving extra rows after line N-1. The renderer then paints
//! `math_lines_below[pos.doc_line]` immediately after the source line,
//! followed by `math_lines_above[pos.doc_line + 1]`, so the final stack
//! reads:
//!
//! ```text
//! <source line N-1>
//! <math_lines_below[N-1] rows>   ← N-1's own below annotations
//! <math_lines_above[N] rows>     ← "above N" emulated via "below N-1"
//! <source line N>
//! <math_lines_below[N] rows>
//! ```

use helix_core::text_annotations::LineAnnotation;
use helix_core::Position;
use helix_view::annotations::math::MathLines;
use helix_view::theme::Style;
use helix_view::{Document, Theme};

use crate::ui::document::{LinePos, TextRenderer};
use crate::ui::text_decorations::{row_placement, Decoration, RowPlacement};

pub struct MathAnnotations<'a> {
    lines: &'a MathLines,
    style: Style,
}

impl<'a> MathAnnotations<'a> {
    pub fn new(doc: &'a Document, theme: &Theme) -> Self {
        // Reuse the same virtual-text style the conceal layer uses — the
        // math lines are semantically part of the same concealed layout.
        let style = theme.get("ui.virtual.math");
        let style = if style == Style::default() {
            theme.get("ui.virtual.conceal")
        } else {
            style
        };
        Self {
            lines: doc.math_lines(),
            style,
        }
    }
}

impl LineAnnotation for MathAnnotations<'_> {
    fn insert_virtual_lines(
        &mut self,
        _line_end_char_idx: usize,
        _line_end_visual_pos: Position,
        doc_line: usize,
    ) -> Position {
        Position::new(self.lines.rows_to_reserve_after(doc_line), 0)
    }
}

impl Decoration for MathAnnotations<'_> {
    fn render_virt_lines(
        &mut self,
        renderer: &mut TextRenderer,
        pos: LinePos,
        virt_off: Position,
    ) -> Position {
        let below = self.lines.below(pos.doc_line).unwrap_or(&[]);
        let above_next = self.lines.above(pos.doc_line + 1).unwrap_or(&[]);

        if below.is_empty() && above_next.is_empty() {
            return Position::new(0, 0);
        }

        let base_row = pos.visual_line + virt_off.row as u16;
        let viewport_height = renderer.viewport.height;
        let offset_row = renderer.offset.row as u16;
        let mut rows_used: u16 = 0;

        for line in below.iter() {
            let block_row = base_row + rows_used;
            match row_placement(block_row, offset_row, viewport_height) {
                RowPlacement::Above => {
                    rows_used += 1;
                    continue;
                }
                RowPlacement::Below => break,
                RowPlacement::Visible => {}
            }
            renderer.set_string(0, block_row, line, self.style);
            rows_used += 1;
        }

        for line in above_next.iter() {
            let block_row = base_row + rows_used;
            match row_placement(block_row, offset_row, viewport_height) {
                RowPlacement::Above => {
                    rows_used += 1;
                    continue;
                }
                RowPlacement::Below => break,
                RowPlacement::Visible => {}
            }
            renderer.set_string(0, block_row, line, self.style);
            rows_used += 1;
        }

        Position::new(rows_used as usize, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_math(offset_row: usize) -> Vec<String> {
        use arc_swap::ArcSwap;
        use helix_core::{syntax, Rope};
        use helix_view::editor::Config;
        use helix_view::graphics::Rect;
        use std::sync::Arc;
        use tui::buffer::{Buffer as Surface, RawSurface};

        let mut doc = Document::from(
            Rope::from_str("cell\nafter\n"),
            None,
            Arc::new(ArcSwap::new(Arc::new(Config::default()))),
            Arc::new(ArcSwap::from_pointee(syntax::Loader::default())),
        );
        doc.set_math_lines_below(0, (0..6).map(|i| format!("M{i}")).collect());

        let theme = Theme::default();
        let viewport = Rect::new(0, 0, 40, 20);
        let mut surface = Surface::empty(viewport);
        let mut raw = RawSurface::new();
        let offset = Position::new(offset_row, 0);
        let mut renderer =
            TextRenderer::new(&mut surface, &mut raw, &doc, &theme, offset, viewport);

        let mut anno = MathAnnotations::new(&doc, &theme);
        let pos = LinePos {
            first_visual_line: true,
            doc_line: 0,
            visual_line: 0,
        };
        anno.render_virt_lines(&mut renderer, pos, Position::new(1, 0));

        (0..viewport.height)
            .map(|y| {
                (0..viewport.width)
                    .map(|x| surface.get(x, y).map_or(" ", |c| &*c.symbol))
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn math_rows_land_below_anchor_without_scroll() {
        let rows = render_math(0);
        assert_eq!(rows[1], "M0");
        assert_eq!(rows[2], "M1");
        assert_eq!(rows[6], "M5");
    }

    #[test]
    fn math_rows_shift_up_by_offset_when_scrolled_into_virtual_region() {
        let rows = render_math(3);
        assert_eq!(rows[0], "M2", "screen row 0");
        assert_eq!(rows[1], "M3", "screen row 1");
        assert_eq!(rows[2], "M4", "screen row 2");
        assert_eq!(rows[3], "M5", "screen row 3");
        for (y, row) in rows.iter().enumerate() {
            assert!(!row.contains("M0"), "M0 leaked at row {y}: {row:?}");
            assert!(!row.contains("M1"), "M1 leaked at row {y}: {row:?}");
        }
    }
}
