use std::collections::HashMap;

use helix_core::text_annotations::{Overlay, SortedRawContent};
use helix_core::{Assoc, ChangeSet, Rope};

use crate::annotations::math::MathLines;
use crate::annotations::output::OutputLines;
use crate::annotations::stale_tags::StaleTags;
use crate::ViewId;

#[derive(Debug, Default, Clone)]
pub struct DisplayLayers {
    pub(crate) output_lines: OutputLines,
    pub(crate) math_lines: MathLines,
    pub(crate) stale_tags: StaleTags,
    pub(crate) plugin_overlays: HashMap<ViewId, Vec<Overlay>>,
    pub(crate) raw_content: HashMap<ViewId, SortedRawContent>,
}

impl DisplayLayers {
    pub fn remap(&mut self, old_doc: &Rope, changes: &ChangeSet, new_text: &Rope) {
        let old_len = old_doc.len_chars();
        let new_len = new_text.len_chars();

        for raw_contents in self.raw_content.values_mut() {
            if raw_contents.is_empty() {
                continue;
            }
            raw_contents.remap_positions(old_len, new_len, |rcs| {
                changes.update_positions(rcs.iter_mut().map(|rc| (&mut rc.char_idx, Assoc::After)));
            });
        }

        for overlays in self.plugin_overlays.values_mut() {
            if overlays.is_empty() {
                continue;
            }
            overlays.retain(|overlay| overlay.char_idx <= old_len);
            overlays.sort_by_key(|overlay| overlay.char_idx);
            changes.update_positions(
                overlays
                    .iter_mut()
                    .map(|overlay| (&mut overlay.char_idx, Assoc::After)),
            );
            overlays.retain(|overlay| overlay.char_idx < new_len);
            overlays.sort_by_key(|overlay| overlay.char_idx);
        }

        if self.output_lines.is_empty() && self.stale_tags.is_empty() && self.math_lines.is_empty()
        {
            return;
        }
        let old_line_count = old_doc.len_lines();
        let remap = |line: usize| {
            if line >= old_line_count {
                return None;
            }
            let mut char_idx = old_doc.line_to_char(line);
            changes.update_positions(std::iter::once((&mut char_idx, Assoc::After)));
            if char_idx > new_len {
                return None;
            }
            Some(new_text.char_to_line(char_idx))
        };
        self.output_lines.remap_lines(remap);
        self.stale_tags.remap_lines(remap);
        self.math_lines.remap_lines(remap);
    }
}

#[cfg(test)]
mod tests {
    use helix_core::text_annotations::{Overlay, RawContent, SortedRawContent};
    use helix_core::{Rope, Tendril, Transaction};

    use crate::annotations::output::{OutputRow, StyledSpan};
    use crate::ViewId;

    use super::DisplayLayers;

    const ANCHOR_LINE: usize = 3;
    const ANCHOR_CHAR: usize = 9;

    fn text() -> Rope {
        Rope::from("ab\ncd\nef\ngh\nij\n")
    }

    fn populate(view: ViewId) -> DisplayLayers {
        let mut layers = DisplayLayers::default();
        layers.output_lines.set_below(
            ANCHOR_LINE,
            vec![OutputRow::new(vec![StyledSpan::from("o")])],
        );
        layers
            .math_lines
            .set_below(ANCHOR_LINE, vec!["m".to_string()]);
        layers.stale_tags.set(ANCHOR_LINE, "s".to_string());

        layers
            .plugin_overlays
            .insert(view, vec![Overlay::new(ANCHOR_CHAR, "P")]);
        let mut raw = SortedRawContent::new();
        raw.push(RawContent::new(ANCHOR_CHAR, 1, vec![], 1));
        layers.raw_content.insert(view, raw);
        layers
    }

    fn edited<I>(base: &Rope, changes: I) -> (Rope, helix_core::ChangeSet)
    where
        I: Iterator<Item = (usize, usize, Option<Tendril>)>,
    {
        let transaction = Transaction::change(base, changes);
        let changeset = transaction.changes().clone();
        let mut new = base.clone();
        assert!(changeset.apply(&mut new));
        (new, changeset)
    }

    fn assert_all_layers(layers: &DisplayLayers, view: ViewId, line: usize, char_idx: usize) {
        assert!(
            layers.output_lines.below(line).is_some(),
            "output row followed the text to line {line}"
        );
        assert!(
            layers.math_lines.below(line).is_some(),
            "math row followed the text to line {line}"
        );
        assert_eq!(
            layers.stale_tags.tag(line),
            Some("s"),
            "stale tag followed the text to line {line}"
        );
        let overlays = layers.plugin_overlays.get(&view).expect("overlay survives");
        assert_eq!(overlays.len(), 1);
        assert_eq!(overlays[0].char_idx, char_idx, "overlay followed the text");
        let raw = layers.raw_content.get(&view).expect("raw content survives");
        assert_eq!(raw.len(), 1);
        assert_eq!(
            raw.as_slice()[0].char_idx,
            char_idx,
            "raw content followed the text"
        );
    }

    #[test]
    fn remap_moves_every_layer_with_the_text() {
        let view = ViewId::default();
        let base = text();

        let insert_above_every_anchor = std::iter::once((0, 0, Some(Tendril::from("XY\n"))));
        let mut layers = populate(view);
        let (new, changes) = edited(&base, insert_above_every_anchor);
        layers.remap(&base, &changes, &new);
        assert_all_layers(&layers, view, 4, 12);

        let delete_range_spanning_every_anchor = std::iter::once((6, 12, None));
        let mut layers = populate(view);
        let (new, changes) = edited(&base, delete_range_spanning_every_anchor);
        layers.remap(&base, &changes, &new);
        assert_all_layers(&layers, view, 2, 6);

        let insert_past_the_end = std::iter::once((15, 15, Some(Tendril::from("ZZ"))));
        let mut layers = populate(view);
        let (new, changes) = edited(&base, insert_past_the_end);
        layers.remap(&base, &changes, &new);
        assert_all_layers(&layers, view, 3, 9);
    }

    #[test]
    fn remap_drops_overlay_at_or_past_new_end() {
        let view = ViewId::default();
        let base = Rope::from("abcdef\n");
        let mut layers = DisplayLayers::default();
        let survives = Overlay::new(1, "a");
        let lands_at_new_end_after_delete = Overlay::new(5, "z");
        layers
            .plugin_overlays
            .insert(view, vec![survives, lands_at_new_end_after_delete]);

        let delete_down_to_length_three = std::iter::once((3, 7, None));
        let (new, changes) = edited(&base, delete_down_to_length_three);
        layers.remap(&base, &changes, &new);

        let overlays = layers.plugin_overlays.get(&view).expect("map entry kept");
        assert_eq!(
            overlays.len(),
            1,
            "overlay at or past the new end was dropped"
        );
        assert_eq!(overlays[0].char_idx, 1);
    }
}
