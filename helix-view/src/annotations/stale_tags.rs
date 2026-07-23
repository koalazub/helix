//! `LineAnnotation` that reserves virtual rows BELOW a source line for a
//! stale-cell marker (nothelix `@param` downstream-invalidation tags). A
//! sibling of [`super::math::MathLines`] and [`super::output::OutputLines`],
//! kept fully independent so stale-tag re-scans never collide with math or
//! output rows and vice-versa. Rendered by
//! `helix_term::ui::text_decorations::stale_tag_annotations::StaleTagAnnotations`.

use std::collections::HashMap;

use helix_core::text_annotations::LineAnnotation;
use helix_core::Position;

use crate::Document;

#[derive(Debug, Default, Clone)]
pub struct StaleTags {
    tags: HashMap<usize, String>,
    above: HashMap<usize, String>,
}

impl StaleTags {
    pub fn is_empty(&self) -> bool {
        self.tags.is_empty() && self.above.is_empty()
    }

    pub fn tag(&self, line_idx: usize) -> Option<&str> {
        self.tags.get(&line_idx).map(String::as_str)
    }

    pub fn tag_above(&self, line_idx: usize) -> Option<&str> {
        self.above.get(&line_idx).map(String::as_str)
    }

    pub fn set(&mut self, line_idx: usize, text: String) {
        if text.is_empty() {
            self.tags.remove(&line_idx);
        } else {
            self.tags.insert(line_idx, text);
        }
    }

    pub fn set_above(&mut self, line_idx: usize, text: String) {
        if text.is_empty() {
            self.above.remove(&line_idx);
        } else {
            self.above.insert(line_idx, text);
        }
    }

    pub fn clear_at(&mut self, line_idx: usize) {
        self.tags.remove(&line_idx);
        self.above.remove(&line_idx);
    }

    pub fn clear(&mut self) {
        self.tags.clear();
        self.above.clear();
    }

    pub fn remap_lines(&mut self, f: impl Fn(usize) -> Option<usize>) {
        for bucket in [&mut self.tags, &mut self.above] {
            if bucket.is_empty() {
                continue;
            }
            let old = std::mem::take(bucket);
            for (line, text) in old {
                if let Some(new_line) = f(line) {
                    bucket.insert(new_line, text);
                }
            }
        }
    }

    pub fn rows_to_reserve_after(&self, doc_line: usize) -> usize {
        usize::from(self.tags.contains_key(&doc_line))
            + usize::from(self.above.contains_key(&(doc_line + 1)))
    }
}

pub struct StaleTagAnnotations<'a> {
    tags: &'a StaleTags,
}

impl<'a> StaleTagAnnotations<'a> {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(doc: &'a Document) -> Box<dyn LineAnnotation + 'a> {
        Box::new(StaleTagAnnotations {
            tags: doc.stale_tags(),
        })
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

#[cfg(test)]
mod tests {
    use super::StaleTags;

    #[test]
    fn empty_by_default() {
        let t = StaleTags::default();
        assert!(t.is_empty());
        assert_eq!(t.tag(3), None);
        assert_eq!(t.rows_to_reserve_after(3), 0);
    }

    #[test]
    fn set_get_clear() {
        let mut t = StaleTags::default();
        t.set(5, "stale".to_string());
        assert!(!t.is_empty());
        assert_eq!(t.tag(5), Some("stale"));
        assert_eq!(t.rows_to_reserve_after(5), 1);
        t.clear_at(5);
        assert!(t.is_empty());
        assert_eq!(t.tag(5), None);
    }

    #[test]
    fn empty_text_removes_entry() {
        let mut t = StaleTags::default();
        t.set(2, "x".to_string());
        t.set(2, String::new());
        assert!(t.is_empty());
    }

    #[test]
    fn clear_all() {
        let mut t = StaleTags::default();
        t.set(1, "a".to_string());
        t.set(9, "b".to_string());
        t.clear();
        assert!(t.is_empty());
    }
}
