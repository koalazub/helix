use helix_core::text_annotations::LineAnnotation;
use helix_core::unicode::width::UnicodeWidthStr;
use helix_core::Position;
use helix_view::annotations::output::OutputLines;
use helix_view::theme::Style;
use helix_view::{Document, Theme};

use crate::ui::document::{LinePos, TextRenderer};
use crate::ui::text_decorations::Decoration;

pub struct OutputAnnotations<'a> {
    lines: &'a OutputLines,
    theme: &'a Theme,
    default_style: Style,
}

impl<'a> OutputAnnotations<'a> {
    pub fn new(doc: &'a Document, theme: &'a Theme) -> Self {
        let style = theme.get("ui.virtual.math");
        let default_style = if style == Style::default() {
            theme.get("ui.virtual.conceal")
        } else {
            style
        };
        Self {
            lines: doc.output_lines(),
            theme,
            default_style,
        }
    }

    /// Resolve a span's scope against the active theme, falling back to the
    /// decoration's default output style when the span carries no scope or
    /// the scope doesn't resolve to anything in the theme.
    fn style_for_scope(&self, scope: Option<&str>) -> Style {
        let Some(scope) = scope else {
            return self.default_style;
        };
        let style = self.theme.get(scope);
        if style == Style::default() {
            self.default_style
        } else {
            style
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

        for row in below.iter() {
            let render_row = base_row + rows_used;
            if render_row >= viewport_height {
                break;
            }
            let mut col: u16 = 0;
            for span in row.iter() {
                let style = self.style_for_scope(span.scope.as_deref());
                renderer.set_string(col, render_row, &span.text, style);
                col += span.text.width() as u16;
            }
            rows_used += 1;
        }

        Position::new(rows_used as usize, 0)
    }
}
