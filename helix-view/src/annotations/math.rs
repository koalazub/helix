//! `LineAnnotation` wrapper that reserves virtual rows around
//! math-bearing source lines.
//!
//! Rendering the rows is handled in `helix_term::ui::text_decorations::
//! MathAnnotations` via the `Decoration` trait. This half only tells the
//! doc formatter how many rows to reserve per source line — the renderer
//! and this reservation use the same lookup into
//! `Document::math_lines_{above,below}`.
//!
//! "Above line N" is emulated as "below line N-1" since Helix only has a
//! reserve-after-source-line hook; see the detailed layout note in the
//! matching `text_decorations::math_annotations` module.

use std::collections::HashMap;

use helix_core::text_annotations::LineAnnotation;
use helix_core::Position;

use crate::Document;

pub struct MathAnnotations<'a> {
    above: &'a HashMap<usize, Vec<String>>,
    below: &'a HashMap<usize, Vec<String>>,
}

impl<'a> MathAnnotations<'a> {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(doc: &'a Document) -> Box<dyn LineAnnotation + 'a> {
        Box::new(MathAnnotations {
            above: &doc.math_lines_above,
            below: &doc.math_lines_below,
        })
    }
}

impl LineAnnotation for MathAnnotations<'_> {
    fn insert_virtual_lines(
        &mut self,
        _line_end_char_idx: usize,
        _line_end_visual_pos: Position,
        doc_line: usize,
    ) -> Position {
        let below_here = self.below.get(&doc_line).map(Vec::len).unwrap_or(0);
        let above_next = self
            .above
            .get(&(doc_line + 1))
            .map(Vec::len)
            .unwrap_or(0);
        Position::new(below_here + above_next, 0)
    }
}
