//! `LineAnnotation` that reserves virtual rows BELOW a source line for
//! notebook cell output (stdout / result / stderr / error). A sibling of
//! [`super::math::MathLines`], kept separate so math re-render
//! (`clear_all_math_lines`) never wipes output and vice-versa. Rendered by
//! `helix_term::ui::text_decorations::output_annotations::OutputAnnotations`.
//!
//! Each row is a sequence of [`StyledSpan`]s rather than a plain `String` so
//! colored output (e.g. per-series braille text-plots) can paint different
//! runs of a row with different theme scopes. A row built from a single
//! plain string (via `StyledSpan::from`) is one no-scope span and renders
//! identically to Plan 1's monochrome output.

use std::collections::HashMap;

use helix_core::text_annotations::LineAnnotation;
use helix_core::Position;

use crate::Document;

/// One styled run of text within an output row. `scope` is a theme scope
/// name (e.g. `"ui.virtual.output.series0"`) resolved against the active
/// theme at render time; `None` paints with the decoration's default output
/// style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledSpan {
    pub text: String,
    pub scope: Option<String>,
}

impl From<String> for StyledSpan {
    fn from(text: String) -> Self {
        StyledSpan { text, scope: None }
    }
}

impl From<&str> for StyledSpan {
    fn from(text: &str) -> Self {
        StyledSpan {
            text: text.to_string(),
            scope: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OutputRow {
    pub bar_scope: Option<String>,
    pub spans: Vec<StyledSpan>,
}

impl OutputRow {
    pub fn new(spans: Vec<StyledSpan>) -> Self {
        OutputRow {
            bar_scope: None,
            spans,
        }
    }

    pub fn with_bar(bar_scope: Option<String>, spans: Vec<StyledSpan>) -> Self {
        OutputRow { bar_scope, spans }
    }
}

impl FromIterator<StyledSpan> for OutputRow {
    fn from_iter<I: IntoIterator<Item = StyledSpan>>(iter: I) -> Self {
        OutputRow::new(iter.into_iter().collect())
    }
}

#[derive(Debug, Default, Clone)]
pub struct OutputLines {
    below: HashMap<usize, Vec<OutputRow>>,
}

impl OutputLines {
    pub fn is_empty(&self) -> bool {
        self.below.is_empty()
    }

    pub fn below(&self, line_idx: usize) -> Option<&[OutputRow]> {
        self.below.get(&line_idx).map(Vec::as_slice)
    }

    pub fn set_below(&mut self, line_idx: usize, lines: Vec<OutputRow>) {
        if lines.is_empty() {
            self.below.remove(&line_idx);
        } else {
            self.below.insert(line_idx, lines);
        }
    }

    pub fn clear_at(&mut self, line_idx: usize) {
        self.below.remove(&line_idx);
    }

    pub fn clear(&mut self) {
        self.below.clear();
    }

    /// Remap every anchor line key through `f`, dropping keys that map to
    /// `None` (the anchor's source line was deleted or pushed past the new
    /// document end). Collisions — two old keys mapping to the same new line —
    /// resolve last-writer-wins; the plugin sets at most one entry per anchor,
    /// so a collision only arises from a pathological edit and either surviving
    /// row set is a defensible choice.
    pub fn remap_lines(&mut self, f: impl Fn(usize) -> Option<usize>) {
        if self.below.is_empty() {
            return;
        }
        let old = std::mem::take(&mut self.below);
        for (line, rows) in old {
            if let Some(new_line) = f(line) {
                self.below.insert(new_line, rows);
            }
        }
    }

    pub fn rows_to_reserve_after(&self, doc_line: usize) -> usize {
        self.below(doc_line).map(<[OutputRow]>::len).unwrap_or(0)
    }
}

pub struct OutputAnnotations<'a> {
    lines: &'a OutputLines,
}

impl<'a> OutputAnnotations<'a> {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(doc: &'a Document) -> Box<dyn LineAnnotation + 'a> {
        Box::new(OutputAnnotations {
            lines: doc.output_lines(),
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styled_row_round_trips_text_and_scopes() {
        let mut lines = OutputLines::default();
        let styled_row = OutputRow::with_bar(
            Some("ui.virtual.output.series1".to_string()),
            vec![
                StyledSpan {
                    text: "hi ".to_string(),
                    scope: Some("ui.virtual.output.series0".to_string()),
                },
                StyledSpan::from("there"),
            ],
        );
        let plain_row = OutputRow::new(vec![StyledSpan::from("plain line".to_string())]);

        lines.set_below(3, vec![styled_row, plain_row]);

        let below = lines.below(3).expect("rows were set for line 3");
        assert_eq!(below.len(), 2);

        let styled_text: String = below[0]
            .spans
            .iter()
            .map(|span| span.text.as_str())
            .collect();
        assert_eq!(styled_text, "hi there");
        assert_eq!(
            below[0].bar_scope.as_deref(),
            Some("ui.virtual.output.series1")
        );
        assert_eq!(
            below[0].spans[0].scope.as_deref(),
            Some("ui.virtual.output.series0")
        );
        assert_eq!(below[0].spans[1].scope, None);

        assert_eq!(below[1].spans.len(), 1);
        assert_eq!(below[1].bar_scope, None);
        assert_eq!(below[1].spans[0].text, "plain line");
        assert_eq!(below[1].spans[0].scope, None);

        assert_eq!(lines.rows_to_reserve_after(3), 2);
    }

    #[test]
    fn remap_lines_shifts_keys_and_drops_none() {
        let mut lines = OutputLines::default();
        lines.set_below(5, vec![OutputRow::new(vec![StyledSpan::from("shifted")])]);
        lines.set_below(2, vec![OutputRow::new(vec![StyledSpan::from("dropped")])]);

        // Simulate an insertion at line 3 that pushes lines >= 3 down by 2,
        // while line 2 (above the edit) is deleted -> None.
        lines.remap_lines(|line| if line >= 3 { Some(line + 2) } else { None });

        assert!(lines.below(5).is_none());
        assert!(lines.below(2).is_none());
        let moved = lines.below(7).expect("line 5 rows moved to line 7");
        assert_eq!(moved[0].spans[0].text, "shifted");
    }
}
