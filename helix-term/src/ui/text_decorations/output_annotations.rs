use helix_core::text_annotations::LineAnnotation;
use helix_core::Position;
use helix_view::annotations::output::OutputLines;
use helix_view::theme::Style;
use helix_view::{Document, Theme};

use crate::ui::document::{LinePos, TextRenderer};
use crate::ui::text_decorations::Decoration;

pub struct OutputAnnotations<'a> {
    lines: &'a OutputLines,
    style: Style,
}

impl<'a> OutputAnnotations<'a> {
    pub fn new(doc: &'a Document, theme: &Theme) -> Self {
        let style = theme.get("ui.virtual.math");
        let style = if style == Style::default() {
            theme.get("ui.virtual.conceal")
        } else {
            style
        };
        Self {
            lines: doc.output_lines(),
            style,
        }
    }
}

impl LineAnnotation for OutputAnnotations<'_> {
    fn insert_virtual_lines(
        &mut self,
        _line_end_char_idx: usize,
        _line_end_visual_pos: Position,
        doc_line: usize,
    ) -> Position {
        Position::new(self.lines.rows_to_reserve_after(doc_line), 0)
    }
}

impl Decoration for OutputAnnotations<'_> {
    fn render_virt_lines(
        &mut self,
        renderer: &mut TextRenderer,
        pos: LinePos,
        virt_off: Position,
    ) -> Position {
        let below = self.lines.below(pos.doc_line).unwrap_or(&[]);

        if below.is_empty() {
            return Position::new(0, 0);
        }

        let base_row = (pos.visual_line + virt_off.row as u16) as u16;
        let viewport_height = renderer.viewport.height;
        let mut rows_used: u16 = 0;

        for line in below.iter() {
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
