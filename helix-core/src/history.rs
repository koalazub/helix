use crate::{Assoc, ChangeSet, Range, Rope, Selection, Transaction};
use once_cell::sync::Lazy;
use regex::Regex;
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct State {
    pub doc: Rope,
    pub selection: Selection,
}

/// Stores the history of changes to a buffer.
///
/// Currently the history is represented as a vector of revisions. The vector
/// always has at least one element: the empty root revision. Each revision
/// with the exception of the root has a parent revision, a [Transaction]
/// that can be applied to its parent to transition from the parent to itself,
/// and an inversion of that transaction to transition from the parent to its
/// latest child.
///
/// When using `u` to undo a change, an inverse of the stored transaction will
/// be applied which will transition the buffer to the parent state.
///
/// Each revision with the exception of the last in the vector also has a
/// last child revision. When using `U` to redo a change, the last child transaction
/// will be applied to the current state of the buffer.
///
/// The current revision is the one currently displayed in the buffer.
///
/// Committing a new revision to the history will update the last child of the
/// current revision, and push a new revision to the end of the vector.
///
/// Revisions are committed with a timestamp. :earlier and :later can be used
/// to jump to the closest revision to a moment in time relative to the timestamp
/// of the current revision plus (:later) or minus (:earlier) the duration
/// given to the command. If a single integer is given, the editor will instead
/// jump the given number of revisions in the vector.
///
/// Limitations:
///  * Changes in selections currently don't commit history changes. The selection
///    will only be updated to the state after a committed buffer change.
///  * The vector of history revisions is currently unbounded. This might
///    cause the memory consumption to grow significantly large during long
///    editing sessions.
///  * Because delete transactions currently don't store the text that they
///    delete, we also store an inversion of the transaction.
///
/// Using time to navigate the history: <https://github.com/helix-editor/helix/pull/194>
#[derive(Debug)]
pub struct History {
    revisions: Vec<Revision>,
    current: usize,
}

/// A single point in history. See [History] for more information.
#[derive(Debug, Clone)]
struct Revision {
    parent: usize,
    last_child: Option<NonZeroUsize>,
    transaction: Transaction,
    // We need an inversion for undos because delete transactions don't store
    // the deleted text.
    inversion: Transaction,
    timestamp: Instant,
    /// Revisions tagged as plugin output are skipped by user-facing undo/redo so
    /// one `u` reverts a user edit together with the output it produced.
    output: bool,
}

impl Default for History {
    fn default() -> Self {
        // Add a dummy root revision with empty transaction
        Self {
            revisions: vec![Revision {
                parent: 0,
                last_child: None,
                transaction: Transaction::from(ChangeSet::new("".into())),
                inversion: Transaction::from(ChangeSet::new("".into())),
                timestamp: Instant::now(),
                output: false,
            }],
            current: 0,
        }
    }
}

impl History {
    pub fn commit_revision(&mut self, transaction: &Transaction, original: &State) {
        self.commit_revision_at_timestamp(transaction, original, Instant::now());
    }

    pub fn commit_revision_at_timestamp(
        &mut self,
        transaction: &Transaction,
        original: &State,
        timestamp: Instant,
    ) {
        let inversion = transaction
            .invert(&original.doc)
            // Store the current cursor position
            .with_selection(original.selection.clone());

        let new_current = self.revisions.len();
        self.revisions[self.current].last_child = NonZeroUsize::new(new_current);
        self.revisions.push(Revision {
            parent: self.current,
            last_child: None,
            transaction: transaction.clone(),
            inversion,
            timestamp,
            output: false,
        });
        self.current = new_current;
    }

    /// Commit a revision tagged as plugin output. Output revisions are skipped by
    /// [`Self::undo_user`]/[`Self::redo_user`] so a single user undo reverts both the
    /// user edit and the output it produced. Behaves identically to
    /// [`Self::commit_revision`] for untagged (`output == false`) revisions.
    pub fn commit_revision_tagged(
        &mut self,
        transaction: &Transaction,
        original: &State,
        output: bool,
    ) {
        self.commit_revision_at_timestamp(transaction, original, Instant::now());
        if let Some(rev) = self.revisions.last_mut() {
            rev.output = output;
        }
    }

    #[inline]
    pub fn current_revision(&self) -> usize {
        self.current
    }

    #[inline]
    pub const fn at_root(&self) -> bool {
        self.current == 0
    }

    /// Returns the changes since the given revision composed into a transaction.
    /// Returns None if there are no changes between the current and given revisions.
    pub fn changes_since(&self, revision: usize) -> Option<Transaction> {
        let lca = self.lowest_common_ancestor(revision, self.current);
        let up = self.path_up(revision, lca);
        let down = self.path_up(self.current, lca);
        let up_txns = up
            .iter()
            .rev()
            .map(|&n| self.revisions[n].inversion.clone());
        let down_txns = down.iter().map(|&n| self.revisions[n].transaction.clone());

        down_txns.chain(up_txns).reduce(|acc, tx| tx.compose(acc))
    }

    /// Undo the last edit.
    pub fn undo(&mut self) -> Option<&Transaction> {
        if self.at_root() {
            return None;
        }

        let current_revision = &self.revisions[self.current];
        self.current = current_revision.parent;
        Some(&current_revision.inversion)
    }

    /// Redo the last edit.
    pub fn redo(&mut self) -> Option<&Transaction> {
        let current_revision = &self.revisions[self.current];
        let last_child = current_revision.last_child?;
        self.current = last_child.get();

        Some(&self.revisions[last_child.get()].transaction)
    }

    /// User-facing undo: revert output-tagged revisions together with the first
    /// non-output (user) revision they follow, so a single `u` never lands on an
    /// output-only state. Returns the inversions to apply in order, or `None` at
    /// root. For a history with no tagged revisions this reverts exactly one
    /// revision, identical to [`Self::undo`].
    pub fn undo_user(&mut self) -> Option<Vec<Transaction>> {
        if self.at_root() {
            return None;
        }
        let mut node = self.current;
        loop {
            let output = self.revisions[node].output;
            node = self.revisions[node].parent;
            if !output || node == 0 {
                break;
            }
        }
        Some(self.jump_to(node))
    }

    /// User-facing redo: restore one non-output (user) revision plus any
    /// output-tagged revisions that immediately follow it, stopping before the
    /// next user revision. Returns the transactions to apply in order, or `None`
    /// if there is nothing to redo. For a history with no tagged revisions this
    /// restores exactly one revision, identical to [`Self::redo`].
    pub fn redo_user(&mut self) -> Option<Vec<Transaction>> {
        let mut node = self.current;
        let in_unit = self.revisions[node]
            .last_child
            .is_some_and(|c| self.revisions[c.get()].output);
        let mut target = None;
        let mut seen_user = false;
        loop {
            let child = match self.revisions[node].last_child {
                Some(c) => c.get(),
                None => break,
            };
            let child_output = self.revisions[child].output;
            if in_unit {
                if !child_output {
                    break;
                }
            } else if seen_user && !child_output {
                break;
            }
            node = child;
            target = Some(node);
            if !child_output {
                seen_user = true;
            }
        }
        target.map(|t| self.jump_to(t))
    }

    // Get the position of last change
    pub fn last_edit_pos(&self) -> Option<usize> {
        if self.current == 0 {
            return None;
        }
        let current_revision = &self.revisions[self.current];
        let primary_selection = current_revision
            .inversion
            .selection()
            .expect("inversion always contains a selection")
            .primary();
        let (_from, to, _fragment) = current_revision
            .transaction
            .changes_iter()
            // find a change that matches the primary selection
            .find(|(from, to, _fragment)| Range::new(*from, *to).overlaps(&primary_selection))
            // or use the first change
            .or_else(|| current_revision.transaction.changes_iter().next())
            .unwrap();
        let pos = current_revision
            .transaction
            .changes()
            .map_pos(to, Assoc::After);
        Some(pos)
    }

    fn lowest_common_ancestor(&self, mut a: usize, mut b: usize) -> usize {
        use std::collections::HashSet;
        let mut a_path_set = HashSet::new();
        let mut b_path_set = HashSet::new();
        loop {
            a_path_set.insert(a);
            b_path_set.insert(b);
            if a_path_set.contains(&b) {
                return b;
            }
            if b_path_set.contains(&a) {
                return a;
            }
            a = self.revisions[a].parent; // Relies on the parent of 0 being 0.
            b = self.revisions[b].parent; // Same as above.
        }
    }

    /// List of nodes on the way from `n` to 'a`. Doesn't include `a`.
    /// Includes `n` unless `a == n`. `a` must be an ancestor of `n`.
    fn path_up(&self, mut n: usize, a: usize) -> Vec<usize> {
        let mut path = Vec::new();
        while n != a {
            path.push(n);
            n = self.revisions[n].parent;
        }
        path
    }

    /// Create a [`Transaction`] that will jump to a specific revision in the history.
    fn jump_to(&mut self, to: usize) -> Vec<Transaction> {
        let lca = self.lowest_common_ancestor(self.current, to);
        let up = self.path_up(self.current, lca);
        let down = self.path_up(to, lca);
        self.current = to;
        let up_txns = up.iter().map(|&n| self.revisions[n].inversion.clone());
        let down_txns = down
            .iter()
            .rev()
            .map(|&n| self.revisions[n].transaction.clone());
        up_txns.chain(down_txns).collect()
    }

    /// Creates a [`Transaction`] that will undo `delta` revisions.
    fn jump_backward(&mut self, delta: usize) -> Vec<Transaction> {
        self.jump_to(self.current.saturating_sub(delta))
    }

    /// Creates a [`Transaction`] that will redo `delta` revisions.
    fn jump_forward(&mut self, delta: usize) -> Vec<Transaction> {
        self.jump_to(
            self.current
                .saturating_add(delta)
                .min(self.revisions.len() - 1),
        )
    }

    /// Helper for a binary search case below.
    fn revision_closer_to_instant(&self, i: usize, instant: Instant) -> usize {
        let dur_im1 = instant.duration_since(self.revisions[i - 1].timestamp);
        let dur_i = self.revisions[i].timestamp.duration_since(instant);
        use std::cmp::Ordering::*;
        match dur_im1.cmp(&dur_i) {
            Less => i - 1,
            Equal | Greater => i,
        }
    }

    /// Creates a [`Transaction`] that will match a revision created at around
    /// `instant`.
    fn jump_instant(&mut self, instant: Instant) -> Vec<Transaction> {
        let search_result = self
            .revisions
            .binary_search_by(|rev| rev.timestamp.cmp(&instant));
        let revision = match search_result {
            Ok(revision) => revision,
            Err(insert_point) => match insert_point {
                0 => 0,
                n if n == self.revisions.len() => n - 1,
                i => self.revision_closer_to_instant(i, instant),
            },
        };
        self.jump_to(revision)
    }

    /// Creates a [`Transaction`] that will match a revision created `duration` ago
    /// from the timestamp of current revision.
    fn jump_duration_backward(&mut self, duration: Duration) -> Vec<Transaction> {
        match self.revisions[self.current].timestamp.checked_sub(duration) {
            Some(instant) => self.jump_instant(instant),
            None => self.jump_to(0),
        }
    }

    /// Creates a [`Transaction`] that will match a revision created `duration` in
    /// the future from the timestamp of the current revision.
    fn jump_duration_forward(&mut self, duration: Duration) -> Vec<Transaction> {
        match self.revisions[self.current].timestamp.checked_add(duration) {
            Some(instant) => self.jump_instant(instant),
            None => self.jump_to(self.revisions.len() - 1),
        }
    }

    /// Creates an undo [`Transaction`].
    pub fn earlier(&mut self, uk: UndoKind) -> Vec<Transaction> {
        use UndoKind::*;
        match uk {
            Steps(n) => self.jump_backward(n),
            TimePeriod(d) => self.jump_duration_backward(d),
        }
    }

    /// Creates a redo [`Transaction`].
    pub fn later(&mut self, uk: UndoKind) -> Vec<Transaction> {
        use UndoKind::*;
        match uk {
            Steps(n) => self.jump_forward(n),
            TimePeriod(d) => self.jump_duration_forward(d),
        }
    }
}

/// Whether to undo by a number of edits or a duration of time.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum UndoKind {
    Steps(usize),
    TimePeriod(std::time::Duration),
}

/// A subset of systemd.time time span syntax units.
const TIME_UNITS: &[(&[&str], &str, u64)] = &[
    (&["seconds", "second", "sec", "s"], "seconds", 1),
    (&["minutes", "minute", "min", "m"], "minutes", 60),
    (&["hours", "hour", "hr", "h"], "hours", 60 * 60),
    (&["days", "day", "d"], "days", 24 * 60 * 60),
];

/// Checks if the duration input can be turned into a valid duration. It must be a
/// positive integer and denote the [unit of time.](`TIME_UNITS`)
/// Examples of valid durations:
///  * `5 sec`
///  * `5 min`
///  * `5 hr`
///  * `5 days`
static DURATION_VALIDATION_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(?:\d+\s*[a-z]+\s*)+$").unwrap());

/// Captures both the number and unit as separate capture groups.
static NUMBER_UNIT_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(\d+)\s*([a-z]+)").unwrap());

/// Parse a string (e.g. "5 sec") and try to convert it into a [`Duration`].
fn parse_human_duration(s: &str) -> Result<Duration, String> {
    if !DURATION_VALIDATION_REGEX.is_match(s) {
        return Err("duration should be composed \
        of positive integers followed by time units"
            .to_string());
    }

    let mut specified = [false; TIME_UNITS.len()];
    let mut seconds = 0u64;
    for cap in NUMBER_UNIT_REGEX.captures_iter(s) {
        let (n, unit_str) = (&cap[1], &cap[2]);

        let n: u64 = n.parse().map_err(|_| format!("integer too large: {}", n))?;

        let time_unit = TIME_UNITS
            .iter()
            .enumerate()
            .find(|(_, (forms, _, _))| forms.iter().any(|f| f == &unit_str));

        if let Some((i, (_, unit, mul))) = time_unit {
            if specified[i] {
                return Err(format!("{} specified more than once", unit));
            }
            specified[i] = true;

            let new_seconds = n.checked_mul(*mul).and_then(|s| seconds.checked_add(s));
            match new_seconds {
                Some(ns) => seconds = ns,
                None => return Err("duration too large".to_string()),
            }
        } else {
            return Err(format!("incorrect time unit: {}", unit_str));
        }
    }

    Ok(Duration::from_secs(seconds))
}

impl std::str::FromStr for UndoKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.is_empty() {
            Ok(Self::Steps(1usize))
        } else if let Ok(n) = s.parse::<usize>() {
            Ok(UndoKind::Steps(n))
        } else {
            Ok(Self::TimePeriod(parse_human_duration(s)?))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::Selection;

    #[test]
    fn test_undo_redo() {
        let mut history = History::default();
        let doc = Rope::from("hello");
        let mut state = State {
            doc,
            selection: Selection::point(0),
        };

        let transaction1 =
            Transaction::change(&state.doc, vec![(5, 5, Some(" world!".into()))].into_iter());

        // Need to commit before applying!
        history.commit_revision(&transaction1, &state);
        transaction1.apply(&mut state.doc);
        assert_eq!("hello world!", state.doc);

        // ---

        let transaction2 =
            Transaction::change(&state.doc, vec![(6, 11, Some("世界".into()))].into_iter());

        // Need to commit before applying!
        history.commit_revision(&transaction2, &state);
        transaction2.apply(&mut state.doc);
        assert_eq!("hello 世界!", state.doc);

        // ---
        fn undo(history: &mut History, state: &mut State) {
            if let Some(transaction) = history.undo() {
                transaction.apply(&mut state.doc);
            }
        }
        fn redo(history: &mut History, state: &mut State) {
            if let Some(transaction) = history.redo() {
                transaction.apply(&mut state.doc);
            }
        }

        undo(&mut history, &mut state);
        assert_eq!("hello world!", state.doc);
        redo(&mut history, &mut state);
        assert_eq!("hello 世界!", state.doc);
        undo(&mut history, &mut state);
        undo(&mut history, &mut state);
        assert_eq!("hello", state.doc);

        // undo at root is a no-op
        undo(&mut history, &mut state);
        assert_eq!("hello", state.doc);
    }

    #[test]
    fn test_earlier_later() {
        let mut history = History::default();
        let doc = Rope::from("a\n");
        let mut state = State {
            doc,
            selection: Selection::point(0),
        };

        fn undo(history: &mut History, state: &mut State) {
            if let Some(transaction) = history.undo() {
                transaction.apply(&mut state.doc);
            }
        }

        fn earlier(history: &mut History, state: &mut State, uk: UndoKind) {
            let txns = history.earlier(uk);
            for txn in txns {
                txn.apply(&mut state.doc);
            }
        }

        fn later(history: &mut History, state: &mut State, uk: UndoKind) {
            let txns = history.later(uk);
            for txn in txns {
                txn.apply(&mut state.doc);
            }
        }

        fn commit_change(
            history: &mut History,
            state: &mut State,
            change: crate::transaction::Change,
            instant: Instant,
        ) {
            let txn = Transaction::change(&state.doc, vec![change].into_iter());
            history.commit_revision_at_timestamp(&txn, state, instant);
            txn.apply(&mut state.doc);
        }

        let t0 = Instant::now();
        let t = |n| t0.checked_add(Duration::from_secs(n)).unwrap();

        commit_change(&mut history, &mut state, (1, 1, Some(" b".into())), t(0));
        assert_eq!("a b\n", state.doc);

        commit_change(&mut history, &mut state, (3, 3, Some(" c".into())), t(10));
        assert_eq!("a b c\n", state.doc);

        commit_change(&mut history, &mut state, (5, 5, Some(" d".into())), t(20));
        assert_eq!("a b c d\n", state.doc);

        undo(&mut history, &mut state);
        assert_eq!("a b c\n", state.doc);

        commit_change(&mut history, &mut state, (5, 5, Some(" e".into())), t(30));
        assert_eq!("a b c e\n", state.doc);

        undo(&mut history, &mut state);
        undo(&mut history, &mut state);
        assert_eq!("a b\n", state.doc);

        commit_change(&mut history, &mut state, (1, 3, None), t(40));
        assert_eq!("a\n", state.doc);

        commit_change(&mut history, &mut state, (1, 1, Some(" f".into())), t(50));
        assert_eq!("a f\n", state.doc);

        use UndoKind::*;

        earlier(&mut history, &mut state, Steps(3));
        assert_eq!("a b c d\n", state.doc);

        later(&mut history, &mut state, TimePeriod(Duration::new(20, 0)));
        assert_eq!("a\n", state.doc);

        earlier(&mut history, &mut state, TimePeriod(Duration::new(19, 0)));
        assert_eq!("a b c d\n", state.doc);

        earlier(
            &mut history,
            &mut state,
            TimePeriod(Duration::new(10000, 0)),
        );
        assert_eq!("a\n", state.doc);

        later(&mut history, &mut state, Steps(50));
        assert_eq!("a f\n", state.doc);

        earlier(&mut history, &mut state, Steps(4));
        assert_eq!("a b c\n", state.doc);

        later(&mut history, &mut state, TimePeriod(Duration::new(1, 0)));
        assert_eq!("a b c\n", state.doc);

        later(&mut history, &mut state, TimePeriod(Duration::new(5, 0)));
        assert_eq!("a b c d\n", state.doc);

        later(&mut history, &mut state, TimePeriod(Duration::new(6, 0)));
        assert_eq!("a b c e\n", state.doc);

        later(&mut history, &mut state, Steps(1));
        assert_eq!("a\n", state.doc);
    }

    #[test]
    fn undo_user_skips_output_revisions() {
        let mut history = History::default();
        let mut state = State {
            doc: Rope::from(""),
            selection: Selection::point(0),
        };

        let t_a = Transaction::change(&state.doc, vec![(0, 0, Some("A".into()))].into_iter());
        history.commit_revision(&t_a, &state);
        t_a.apply(&mut state.doc);

        let t_b = Transaction::change(&state.doc, vec![(1, 1, Some("B".into()))].into_iter());
        history.commit_revision_tagged(&t_b, &state, true);
        t_b.apply(&mut state.doc);

        let txns = history.undo_user().expect("something to undo");
        for t in &txns {
            t.apply(&mut state.doc);
        }
        assert_eq!("", state.doc);
        assert!(history.at_root());
    }

    #[test]
    fn undo_user_stops_after_one_user_revision() {
        let mut history = History::default();
        let mut state = State {
            doc: Rope::from(""),
            selection: Selection::point(0),
        };
        for ch in ["A", "B"] {
            let len = state.doc.len_chars();
            let t = Transaction::change(&state.doc, vec![(len, len, Some(ch.into()))].into_iter());
            history.commit_revision(&t, &state);
            t.apply(&mut state.doc);
        }

        let txns = history.undo_user().unwrap();
        for t in &txns {
            t.apply(&mut state.doc);
        }
        assert_eq!("A", state.doc);
    }

    #[test]
    fn redo_user_restores_user_edit_and_following_output() {
        let mut history = History::default();
        let mut state = State {
            doc: Rope::from(""),
            selection: Selection::point(0),
        };

        let t_a = Transaction::change(&state.doc, vec![(0, 0, Some("A".into()))].into_iter());
        history.commit_revision(&t_a, &state);
        t_a.apply(&mut state.doc);

        let t_b = Transaction::change(&state.doc, vec![(1, 1, Some("B".into()))].into_iter());
        history.commit_revision_tagged(&t_b, &state, true);
        t_b.apply(&mut state.doc);

        for t in &history.undo_user().unwrap() {
            t.apply(&mut state.doc);
        }
        assert_eq!("", state.doc);

        let txns = history.redo_user().unwrap();
        for t in &txns {
            t.apply(&mut state.doc);
        }
        assert_eq!("AB", state.doc);
    }

    #[test]
    fn untagged_history_undo_user_equals_undo() {
        let mut history = History::default();
        let mut state = State {
            doc: Rope::from(""),
            selection: Selection::point(0),
        };

        let t = Transaction::change(&state.doc, vec![(0, 0, Some("X".into()))].into_iter());
        history.commit_revision(&t, &state);
        t.apply(&mut state.doc);

        let txns = history.undo_user().unwrap();
        assert_eq!(1, txns.len());
        for t in &txns {
            t.apply(&mut state.doc);
        }
        assert_eq!("", state.doc);
    }

    fn adv_state() -> State {
        State {
            doc: Rope::from(""),
            selection: Selection::point(0),
        }
    }

    fn adv_append(history: &mut History, state: &mut State, ch: &str, output: bool) {
        let len = state.doc.len_chars();
        let t = Transaction::change(&state.doc, vec![(len, len, Some(ch.into()))].into_iter());
        if output {
            history.commit_revision_tagged(&t, state, true);
        } else {
            history.commit_revision(&t, state);
        }
        t.apply(&mut state.doc);
    }

    fn adv_apply(txns: &[Transaction], state: &mut State) {
        for t in txns {
            t.apply(&mut state.doc);
        }
    }

    #[test]
    fn adv_undo_user_at_root_returns_none() {
        let mut history = History::default();
        assert!(history.undo_user().is_none());
        assert!(history.at_root());
    }

    #[test]
    fn adv_redo_user_nothing_to_redo_returns_none() {
        let mut history = History::default();
        assert!(history.redo_user().is_none());
        let mut state = adv_state();
        adv_append(&mut history, &mut state, "A", false);
        // At the tip, there is nothing to redo.
        assert!(history.redo_user().is_none());
    }

    #[test]
    fn adv_run_of_two_outputs() {
        let mut history = History::default();
        let mut state = adv_state();
        adv_append(&mut history, &mut state, "U", false);
        adv_append(&mut history, &mut state, "O", true);
        adv_append(&mut history, &mut state, "P", true);
        assert_eq!("UOP", state.doc);

        let txns = history.undo_user().unwrap();
        adv_apply(&txns, &mut state);
        assert_eq!("", state.doc);
        assert!(history.at_root());

        let txns = history.redo_user().unwrap();
        adv_apply(&txns, &mut state);
        assert_eq!("UOP", state.doc);
    }

    #[test]
    fn adv_alternating_user_output() {
        let mut history = History::default();
        let mut state = adv_state();
        adv_append(&mut history, &mut state, "1", false);
        adv_append(&mut history, &mut state, "a", true);
        adv_append(&mut history, &mut state, "2", false);
        adv_append(&mut history, &mut state, "b", true);
        assert_eq!("1a2b", state.doc);

        let txns = history.undo_user().unwrap();
        adv_apply(&txns, &mut state);
        assert_eq!("1a", state.doc);

        let txns = history.undo_user().unwrap();
        adv_apply(&txns, &mut state);
        assert_eq!("", state.doc);
        assert!(history.at_root());

        let txns = history.redo_user().unwrap();
        adv_apply(&txns, &mut state);
        assert_eq!("1a", state.doc);

        let txns = history.redo_user().unwrap();
        adv_apply(&txns, &mut state);
        assert_eq!("1a2b", state.doc);
    }

    #[test]
    fn adv_leading_output_first_commit() {
        let mut history = History::default();
        let mut state = adv_state();
        // The very first commit is tagged output (parent is root).
        adv_append(&mut history, &mut state, "O", true);
        assert_eq!("O", state.doc);

        let txns = history.undo_user().unwrap();
        adv_apply(&txns, &mut state);
        assert_eq!("", state.doc);
        assert!(history.at_root());

        // Redo must bring the leading output back.
        let txns = history.redo_user().unwrap();
        adv_apply(&txns, &mut state);
        assert_eq!("O", state.doc);
    }

    #[test]
    fn adv_branching_with_output() {
        let mut history = History::default();
        let mut state = adv_state();
        // First branch: U1, O1.
        adv_append(&mut history, &mut state, "x", false);
        adv_append(&mut history, &mut state, "y", true);
        assert_eq!("xy", state.doc);

        // Undo back to root, then commit a new branch: U2, O2.
        let txns = history.undo_user().unwrap();
        adv_apply(&txns, &mut state);
        assert_eq!("", state.doc);

        adv_append(&mut history, &mut state, "2", false);
        adv_append(&mut history, &mut state, "b", true);
        assert_eq!("2b", state.doc);

        // Undo new branch.
        let txns = history.undo_user().unwrap();
        adv_apply(&txns, &mut state);
        assert_eq!("", state.doc);

        // Redo must follow last_child (the newer branch), not the old one.
        let txns = history.redo_user().unwrap();
        adv_apply(&txns, &mut state);
        assert_eq!("2b", state.doc);
    }

    #[test]
    fn adv_redo_user_from_midrun_via_earlier() {
        // Reach a strictly mid-output-run `current` the way the real editor can:
        // `:earlier` / raw undo (NOT undo_user). Structure: U, O1, O2, U2.
        let mut history = History::default();
        let mut state = adv_state();
        adv_append(&mut history, &mut state, "u", false);
        adv_append(&mut history, &mut state, "1", true);
        adv_append(&mut history, &mut state, "2", true);
        adv_append(&mut history, &mut state, "x", false);
        assert_eq!("u12x", state.doc);

        // Step back two revisions (u12x -> u12 -> u1) landing on O1, mid-run.
        let t = history.undo().unwrap().clone();
        t.apply(&mut state.doc);
        let t = history.undo().unwrap().clone();
        t.apply(&mut state.doc);
        assert_eq!("u1", state.doc);

        // One redo. From a mid-output-run `current`, redo restores only the
        // remaining output(s) of the current unit and stops before the next
        // user edit.
        let txns = history.redo_user().unwrap();
        adv_apply(&txns, &mut state);
        // Restores O2 only; stops before the following user edit U2.
        assert_eq!("u12", state.doc);
    }

    #[test]
    fn test_parse_undo_kind() {
        use UndoKind::*;

        // Default is one step.
        assert_eq!("".parse(), Ok(Steps(1)));

        // An integer means the number of steps.
        assert_eq!("1".parse(), Ok(Steps(1)));
        assert_eq!("  16 ".parse(), Ok(Steps(16)));

        // Duration has a strict format.
        let validation_err = Err("duration should be composed \
         of positive integers followed by time units"
            .to_string());
        assert_eq!("  16 33".parse::<UndoKind>(), validation_err);
        assert_eq!("  seconds 22  ".parse::<UndoKind>(), validation_err);
        assert_eq!("  -4 m".parse::<UndoKind>(), validation_err);
        assert_eq!("5s 3".parse::<UndoKind>(), validation_err);

        // Units are u64.
        assert_eq!(
            "18446744073709551616minutes".parse::<UndoKind>(),
            Err("integer too large: 18446744073709551616".to_string())
        );

        // Units are validated.
        assert_eq!(
            "1 millennium".parse::<UndoKind>(),
            Err("incorrect time unit: millennium".to_string())
        );

        // Units can't be specified twice.
        assert_eq!(
            "2 seconds 6s".parse::<UndoKind>(),
            Err("seconds specified more than once".to_string())
        );

        // Various formats are correctly handled.
        assert_eq!(
            "4s".parse::<UndoKind>(),
            Ok(TimePeriod(Duration::from_secs(4)))
        );
        assert_eq!(
            "2m".parse::<UndoKind>(),
            Ok(TimePeriod(Duration::from_secs(120)))
        );
        assert_eq!(
            "5h".parse::<UndoKind>(),
            Ok(TimePeriod(Duration::from_secs(5 * 60 * 60)))
        );
        assert_eq!(
            "3d".parse::<UndoKind>(),
            Ok(TimePeriod(Duration::from_secs(3 * 24 * 60 * 60)))
        );
        assert_eq!(
            "1m30s".parse::<UndoKind>(),
            Ok(TimePeriod(Duration::from_secs(90)))
        );
        assert_eq!(
            "1m 20 seconds".parse::<UndoKind>(),
            Ok(TimePeriod(Duration::from_secs(80)))
        );
        assert_eq!(
            "  2 minute 1day".parse::<UndoKind>(),
            Ok(TimePeriod(Duration::from_secs(24 * 60 * 60 + 2 * 60)))
        );
        assert_eq!(
            "3 d 2hour 5 minutes 30sec".parse::<UndoKind>(),
            Ok(TimePeriod(Duration::from_secs(
                3 * 24 * 60 * 60 + 2 * 60 * 60 + 5 * 60 + 30
            )))
        );

        // Sum overflow is handled.
        assert_eq!(
            "18446744073709551615minutes".parse::<UndoKind>(),
            Err("duration too large".to_string())
        );
        assert_eq!(
            "1 minute 18446744073709551615 seconds".parse::<UndoKind>(),
            Err("duration too large".to_string())
        );
    }
}
