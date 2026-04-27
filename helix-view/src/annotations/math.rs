//! `LineAnnotation` wrapper that reserves virtual rows around
//! math-bearing source lines, plus the [`MathLines`] storage type that
//! backs it.
//!
//! Rendering the rows is handled in `helix_term::ui::text_decorations::
//! MathAnnotations` via the `Decoration` trait. This half only tells the
//! doc formatter how many rows to reserve per source line — both the
//! reservation and the renderer read from the same [`MathLines`] store
//! returned by [`crate::Document::math_lines`].
//!
//! "Above line N" is emulated as "below line N-1" since Helix only has a
//! reserve-after-source-line hook; see the detailed layout note in the
//! matching `text_decorations::math_annotations` module.

use std::collections::HashMap;

use helix_core::text_annotations::LineAnnotation;
use helix_core::Position;

use crate::Document;

/// Plugin-managed virtual lines that render above and below source
/// lines for math layouts. Keyed by 0-based source line index; each
/// entry is a list of pre-padded virtual-line strings (the producing
/// plugin owns leading whitespace so columns align with the source
/// glyph). Used by the nothelix math renderer to stack `\int_0^1` →
/// `1` above, `0` below, keeping the original source line intact.
///
/// Storage is private; readers go through [`Self::above`] /
/// [`Self::below`] / [`Self::is_empty`] and writers through
/// [`Self::set_above`] / [`Self::set_below`] / [`Self::clear_at`] /
/// [`Self::clear`]. The intent is that the only mutators are the
/// `Document::set_math_lines_*` methods (which delegate here), so
/// invariants stay within this module.
#[derive(Debug, Default, Clone)]
pub struct MathLines {
    above: HashMap<usize, Vec<String>>,
    below: HashMap<usize, Vec<String>>,
}

impl MathLines {
    pub fn is_empty(&self) -> bool {
        self.above.is_empty() && self.below.is_empty()
    }

    /// Lines to render above the given source line index, if any.
    pub fn above(&self, line_idx: usize) -> Option<&[String]> {
        self.above.get(&line_idx).map(Vec::as_slice)
    }

    /// Lines to render below the given source line index, if any.
    pub fn below(&self, line_idx: usize) -> Option<&[String]> {
        self.below.get(&line_idx).map(Vec::as_slice)
    }

    /// Replace the above-line bucket for `line_idx`. An empty `lines`
    /// removes the entry entirely so [`Self::is_empty`] stays accurate.
    pub fn set_above(&mut self, line_idx: usize, lines: Vec<String>) {
        if lines.is_empty() {
            self.above.remove(&line_idx);
        } else {
            self.above.insert(line_idx, lines);
        }
    }

    /// Mirror of [`Self::set_above`] for the below bucket.
    pub fn set_below(&mut self, line_idx: usize, lines: Vec<String>) {
        if lines.is_empty() {
            self.below.remove(&line_idx);
        } else {
            self.below.insert(line_idx, lines);
        }
    }

    /// Drop both above and below buckets for a single source line.
    pub fn clear_at(&mut self, line_idx: usize) {
        self.above.remove(&line_idx);
        self.below.remove(&line_idx);
    }

    /// Wipe every annotation. Called when a producing plugin re-runs
    /// its renderer from scratch.
    pub fn clear(&mut self) {
        self.above.clear();
        self.below.clear();
    }

    /// Total virtual rows the formatter must reserve after `doc_line`:
    /// that line's own below bucket plus the next line's above bucket
    /// (emulated here because Helix only has a reserve-after-line
    /// hook).
    pub fn rows_to_reserve_after(&self, doc_line: usize) -> usize {
        let below_here = self.below(doc_line).map(<[String]>::len).unwrap_or(0);
        let above_next = self.above(doc_line + 1).map(<[String]>::len).unwrap_or(0);
        below_here + above_next
    }
}

pub struct MathAnnotations<'a> {
    lines: &'a MathLines,
}

impl<'a> MathAnnotations<'a> {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(doc: &'a Document) -> Box<dyn LineAnnotation + 'a> {
        Box::new(MathAnnotations {
            lines: doc.math_lines(),
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
        Position::new(self.lines.rows_to_reserve_after(doc_line), 0)
    }
}
