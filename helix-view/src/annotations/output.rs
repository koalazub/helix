//! `LineAnnotation` that reserves virtual rows BELOW a source line for
//! notebook cell output (stdout / result / stderr / error). A sibling of
//! [`super::math::MathLines`], kept separate so math re-render
//! (`clear_all_math_lines`) never wipes output and vice-versa. Rendered by
//! `helix_term::ui::text_decorations::output_annotations::OutputAnnotations`.

use std::collections::HashMap;

use helix_core::text_annotations::LineAnnotation;
use helix_core::Position;

use crate::Document;

#[derive(Debug, Default, Clone)]
pub struct OutputLines {
    below: HashMap<usize, Vec<String>>,
}

impl OutputLines {
    pub fn is_empty(&self) -> bool {
        self.below.is_empty()
    }

    pub fn below(&self, line_idx: usize) -> Option<&[String]> {
        self.below.get(&line_idx).map(Vec::as_slice)
    }

    pub fn set_below(&mut self, line_idx: usize, lines: Vec<String>) {
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

    pub fn rows_to_reserve_after(&self, doc_line: usize) -> usize {
        self.below(doc_line).map(<[String]>::len).unwrap_or(0)
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
