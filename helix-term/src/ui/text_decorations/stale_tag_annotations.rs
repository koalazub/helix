//! Stale-tag virtual rows. Helix only reserves rows AFTER a source line, so a
//! tag for line N renders on the row immediately below line N. This layer is
//! independent of math-lines and paints after it (see editor.rs decoration
//! order), so it stacks below any math rows rather than over them.

use helix_core::text_annotations::LineAnnotation;
use helix_core::Position;
use helix_view::annotations::stale_tags::StaleTags;
use helix_view::theme::Style;
use helix_view::{Document, Theme};

use crate::ui::document::{LinePos, TextRenderer};
use crate::ui::text_decorations::Decoration;

pub struct StaleTagAnnotations<'a> {
    tags: &'a StaleTags,
    style: Style,
}

impl<'a> StaleTagAnnotations<'a> {
    pub fn new(doc: &'a Document, theme: &Theme) -> Self {
        let style = theme.get("ui.virtual.stale");
        let style = if style == Style::default() {
            theme.get("ui.virtual.conceal")
        } else {
            style
        };
        Self {
            tags: doc.stale_tags(),
            style,
        }
    }
}

impl LineAnnotation for StaleTagAnnotations<'_> {
    fn insert_virtual_lines(
        &mut self,
        _line_end_char_idx: usize,
        _line_end_visual_pos: Position,
        doc_line: usize,
    ) -> Position {
        Position::new(self.tags.rows_to_reserve_after(doc_line), 0)
    }
}

impl Decoration for StaleTagAnnotations<'_> {
    fn render_virt_lines(
        &mut self,
        renderer: &mut TextRenderer,
        pos: LinePos,
        virt_off: Position,
    ) -> Position {
        let Some(tag) = self.tags.tag(pos.doc_line) else {
            return Position::new(0, 0);
        };

        let base_row = pos.visual_line + virt_off.row as u16;
        if base_row >= renderer.viewport.height {
            return Position::new(0, 0);
        }

        renderer.set_string(0, base_row, tag, self.style);
        Position::new(1, 0)
    }
}
