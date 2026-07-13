use helix_core::text_annotations::LineAnnotation;
use helix_core::unicode::width::UnicodeWidthStr;
use helix_core::Position;
use helix_view::annotations::output::{OutputLines, StyledSpan};
use helix_view::theme::Style;
use helix_view::{Document, Theme};

use crate::ui::document::{LinePos, TextRenderer};
use crate::ui::text_decorations::Decoration;

const BAR_GLYPH: &str = "▏";
const BAR_WIDTH: u16 = 1;

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

        let base_row = pos.visual_line + virt_off.row as u16;
        let viewport_height = renderer.viewport.height;
        let viewport_width = renderer.viewport.width;
        let viewport_x = renderer.viewport.x;
        let mut rows_used: u16 = 0;

        for row in below.iter() {
            let render_row = base_row + rows_used;
            if render_row >= viewport_height {
                break;
            }
            let bar_width = if row.bar_scope.is_some() {
                BAR_WIDTH
            } else {
                0
            };
            if let Some(bar_scope) = row.bar_scope.as_deref() {
                if bar_width <= viewport_width {
                    let style = self.style_for_scope(Some(bar_scope));
                    renderer.set_string_truncated(
                        viewport_x,
                        render_row,
                        BAR_GLYPH,
                        bar_width as usize,
                        |_| style,
                        false,
                        false,
                    );
                }
            }
            for draw in plan_row(&row.spans, bar_width, viewport_width) {
                let style = self.style_for_scope(draw.span.scope.as_deref());
                renderer.set_string_truncated(
                    viewport_x + draw.col,
                    render_row,
                    &draw.span.text,
                    draw.remaining as usize,
                    |_| style,
                    draw.truncated,
                    false,
                );
            }
            rows_used += 1;
        }

        Position::new(rows_used as usize, 0)
    }
}

/// One span's draw op for a single output row, clamped to the text viewport.
/// `remaining` is the cells left from `col` to the right viewport edge;
/// `truncated` marks the span that runs past the edge (drawn with an ellipsis
/// and terminating the row).
struct SpanDraw<'a> {
    span: &'a helix_view::annotations::output::StyledSpan,
    col: u16,
    remaining: u16,
    truncated: bool,
}

fn plan_row(spans: &[StyledSpan], start_col: u16, width: u16) -> Vec<SpanDraw<'_>> {
    let mut plan = Vec::new();
    let mut col: u16 = start_col.min(width);
    for span in spans.iter() {
        if col >= width {
            break;
        }
        let remaining = width - col;
        let span_width = span.text.width() as u16;
        let truncated = span_width > remaining;
        plan.push(SpanDraw {
            span,
            col,
            remaining,
            truncated,
        });
        if truncated {
            break;
        }
        col += span_width;
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans(texts: &[&str]) -> Vec<StyledSpan> {
        texts.iter().map(|t| StyledSpan::from(*t)).collect()
    }

    #[test]
    fn narrow_row_is_untruncated() {
        let r = spans(&["abc", "de"]);
        let plan = plan_row(&r, 0, 80);
        assert_eq!(plan.len(), 2);
        assert!(plan.iter().all(|d| !d.truncated));
        assert_eq!(plan[0].col, 0);
        assert_eq!(plan[1].col, 3);
    }

    #[test]
    fn overrunning_row_truncates_at_edge() {
        let r = spans(&["0123456789", "abcdefghij"]);
        let plan = plan_row(&r, 0, 15);
        assert_eq!(plan.len(), 2);
        assert!(!plan[0].truncated);
        assert!(plan[1].truncated);
        assert_eq!(plan[1].col, 10);
        assert_eq!(plan[1].remaining, 5);
        for d in &plan {
            assert!(d.col + d.remaining <= 15);
        }
    }

    #[test]
    fn spans_past_edge_are_dropped() {
        let r = spans(&["0123456789", "over", "more"]);
        let plan = plan_row(&r, 0, 8);
        assert_eq!(plan.len(), 1);
        assert!(plan[0].truncated);
        assert_eq!(plan[0].col, 0);
        assert_eq!(plan[0].remaining, 8);
    }

    #[test]
    fn single_span_wider_than_viewport_truncates() {
        let r = spans(&["this is a very long single logical output line"]);
        let plan = plan_row(&r, 0, 10);
        assert_eq!(plan.len(), 1);
        assert!(plan[0].truncated);
        assert_eq!(plan[0].remaining, 10);
    }

    #[test]
    fn bar_shifts_text_right_and_stays_within_width() {
        let r = spans(&["hello"]);
        let plan = plan_row(&r, BAR_WIDTH, 80);
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].col, BAR_WIDTH);
        assert_eq!(plan[0].remaining, 80 - BAR_WIDTH);
        assert!(!plan[0].truncated);
        for d in &plan {
            assert!(d.col + BAR_WIDTH <= 80 + BAR_WIDTH);
        }
    }

    #[test]
    fn bar_offset_at_viewport_edge_drops_text() {
        let r = spans(&["x"]);
        let plan = plan_row(&r, BAR_WIDTH, BAR_WIDTH);
        assert_eq!(plan.len(), 1);
        assert!(plan[0].truncated);
        assert_eq!(plan[0].col, BAR_WIDTH);
        assert_eq!(plan[0].remaining, 0);
    }
}
