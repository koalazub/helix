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
use crate::ui::text_decorations::{row_placement, Decoration, RowPlacement};

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
        let below = self.tags.tag(pos.doc_line);
        let above_next = self.tags.tag_above(pos.doc_line + 1);
        if below.is_none() && above_next.is_none() {
            return Position::new(0, 0);
        }

        let offset_row = renderer.offset.row as u16;
        let mut rows_used: u16 = 0;
        for tag in [below, above_next].into_iter().flatten() {
            let row = pos.visual_line + virt_off.row as u16 + rows_used;
            if row_placement(row, offset_row, renderer.viewport.height) == RowPlacement::Visible {
                renderer.set_string(0, row, tag, self.style);
            }
            rows_used += 1;
        }
        Position::new(rows_used as usize, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_stale(offset_row: usize, visual_line: u16) -> Vec<String> {
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
        doc.set_stale_tag(0, "TAG".to_string());

        let theme = Theme::default();
        let viewport = Rect::new(0, 0, 40, 20);
        let mut surface = Surface::empty(viewport);
        let mut raw = RawSurface::new();
        let offset = Position::new(offset_row, 0);
        let mut renderer =
            TextRenderer::new(&mut surface, &mut raw, &doc, &theme, offset, viewport);

        let mut anno = StaleTagAnnotations::new(&doc, &theme);
        let pos = LinePos {
            first_visual_line: true,
            doc_line: 0,
            visual_line,
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
    fn stale_tag_lands_at_block_row_without_scroll() {
        let rows = render_stale(0, 5);
        assert_eq!(rows[6], "TAG");
    }

    #[test]
    fn stale_tag_shifts_up_by_offset_when_scrolled() {
        let rows = render_stale(3, 5);
        assert_eq!(rows[3], "TAG", "screen row 3");
        for (y, row) in rows.iter().enumerate() {
            if y != 3 {
                assert!(!row.contains("TAG"), "TAG leaked at row {y}: {row:?}");
            }
        }
    }

    #[test]
    fn stale_tag_scrolled_above_top_is_not_painted() {
        let rows = render_stale(3, 0);
        for (y, row) in rows.iter().enumerate() {
            assert!(!row.contains("TAG"), "TAG leaked at row {y}: {row:?}");
        }
    }
}
