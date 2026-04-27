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
use crate::ui::text_decorations::Decoration;

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

        let base_row = (pos.visual_line + virt_off.row as u16) as u16;
        let viewport_height = renderer.viewport.height;
        let mut rows_used: u16 = 0;

        // 1. Paint `below[pos.doc_line]` immediately after the source line.
        for line in below.iter() {
            let row = base_row + rows_used;
            if row >= viewport_height {
                break;
            }
            renderer.set_string(0, row, line, self.style);
            rows_used += 1;
        }

        // 2. Paint `above[pos.doc_line + 1]` directly above the next source
        //    line. The rows are emitted immediately after (1) — when the
        //    next source line renders it draws on the row that follows.
        for line in above_next.iter() {
            let row = base_row + rows_used;
            if row >= viewport_height {
                break;
            }
            renderer.set_string(0, row, line, self.style);
            rows_used += 1;
        }

        Position::new(rows_used as usize, 0)
    }
}
