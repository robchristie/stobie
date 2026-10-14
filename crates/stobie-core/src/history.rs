//! Generic undo and redo for application-owned commands.

use std::collections::VecDeque;

/// The capacity of [`History::default`], in undo entries.
pub const DEFAULT_HISTORY_CAPACITY: usize = 100;

/// One reversible change to an application-owned target `T`.
///
/// `T` is whatever the command changes. A command that edits one model uses
/// that model. A command that spans several owners, such as a document, a
/// session and the workspace, uses a small struct of mutable borrows that the
/// application builds at the call site. Commands carry the values they need to
/// apply and revert themselves, so validate an intent against current state
/// before constructing its command.
pub trait Command<T: ?Sized> {
    /// Apply the change. Return `Ok(false)` when it would change nothing:
    /// [`History`] then records nothing and keeps its redo entries. An error
    /// must leave `target` unchanged.
    fn apply(&self, target: &mut T) -> Result<bool, String>;

    /// Reverse an applied change. An error must leave `target` unchanged; the
    /// history entry then stays where it was.
    fn revert(&self, target: &mut T) -> Result<(), String>;

    /// Fold `next`, which has just been applied, into this command so that
    /// both undo as one step. Only [`History::execute_merging`] asks. Return
    /// `false` to keep them separate.
    fn merge(&mut self, next: &Self) -> bool
    where
        Self: Sized,
    {
        let _ = next;
        false
    }

    /// Whether a merged command now changes nothing, such as a run of nudges
    /// that returned to where it started. [`History::execute_merging`] then
    /// drops it rather than keep an undo step that does nothing.
    fn is_noop(&self) -> bool {
        false
    }
}

#[derive(Clone)]
struct Entry<C> {
    /// Identifies the state this entry leaves; see [`History::is_saved`].
    state: u64,
    commands: Vec<C>,
}

/// Bounded undo and redo history over [`Command`]s.
///
/// Each entry is one completed user change: a single command, a group applied
/// together by [`execute_group`](Self::execute_group), or a run of steps folded
/// together by [`execute_merging`](Self::execute_merging). A new change clears
/// redo. When the history is full, the oldest entry is forgotten.
///
/// [`mark_saved`](Self::mark_saved) records a save point, and
/// [`is_saved`](Self::is_saved) reports whether undo and redo have returned to
/// it, so an application can show "unsaved" without comparing whole states.
///
/// ```
/// use stobie_core::{Command, History};
///
/// struct Append(&'static str);
///
/// impl Command<Vec<&'static str>> for Append {
///     fn apply(&self, notes: &mut Vec<&'static str>) -> Result<bool, String> {
///         notes.push(self.0);
///         Ok(true)
///     }
///     fn revert(&self, notes: &mut Vec<&'static str>) -> Result<(), String> {
///         notes.pop();
///         Ok(())
///     }
/// }
///
/// let mut notes = Vec::new();
/// let mut history = History::default();
/// history.execute(Append("first"), &mut notes)?;
/// history.mark_saved();
/// // Two steps of one edit undo together.
/// history.execute_group([Append("second"), Append("third")], &mut notes)?;
/// assert!(!history.is_saved());
/// assert!(history.undo(&mut notes)?);
/// assert_eq!(notes, ["first"]);
/// assert!(history.is_saved());
/// assert!(history.redo(&mut notes)?);
/// assert_eq!(notes, ["first", "second", "third"]);
/// # Ok::<(), String>(())
/// ```
#[derive(Clone)]
pub struct History<C> {
    undo: VecDeque<Entry<C>>,
    redo: Vec<Entry<C>>,
    capacity: usize,
    /// The state with no undo entries left.
    base: u64,
    next_state: u64,
    saved: Option<u64>,
    revision: u64,
    /// Whether the newest entry still accepts merges.
    open: bool,
}

impl<C> Default for History<C> {
    fn default() -> Self {
        Self::new(DEFAULT_HISTORY_CAPACITY)
    }
}

impl<C> std::fmt::Debug for History<C> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("History")
            .field("undo_len", &self.undo.len())
            .field("redo_len", &self.redo.len())
            .field("capacity", &self.capacity)
            .field("saved", &self.is_saved())
            .field("revision", &self.revision)
            .finish()
    }
}

impl<C> History<C> {
    /// An empty history that keeps at most `capacity` undo entries (at least
    /// one). It starts at its save point.
    pub fn new(capacity: usize) -> Self {
        Self {
            undo: VecDeque::new(),
            redo: Vec::new(),
            capacity: capacity.max(1),
            base: 0,
            next_state: 1,
            saved: Some(0),
            revision: 0,
            open: false,
        }
    }

    /// Apply `command` and record it as one undo entry. Returns `Ok(false)`,
    /// recording nothing, when the command changes nothing. An error records
    /// nothing.
    pub fn execute<T: ?Sized>(&mut self, command: C, target: &mut T) -> Result<bool, String>
    where
        C: Command<T>,
    {
        if !command.apply(target)? {
            return Ok(false);
        }
        self.push(vec![command]);
        Ok(true)
    }

    /// Apply several commands in order as one undo entry, such as the steps of
    /// one transaction. Commands that change nothing are dropped. If one
    /// fails, the commands already applied are reverted in reverse order and
    /// the error is returned; nothing is recorded.
    pub fn execute_group<T: ?Sized>(
        &mut self,
        commands: impl IntoIterator<Item = C>,
        target: &mut T,
    ) -> Result<bool, String>
    where
        C: Command<T>,
    {
        let mut applied = Vec::new();
        for command in commands {
            match command.apply(target) {
                Ok(true) => applied.push(command),
                Ok(false) => {}
                Err(error) => {
                    for command in applied.iter().rev() {
                        let _ = command.revert(target);
                    }
                    return Err(error);
                }
            }
        }
        if applied.is_empty() {
            return Ok(false);
        }
        self.push(applied);
        Ok(true)
    }

    /// Apply `command` and, when [`Command::merge`] accepts it, fold it into
    /// the newest entry instead of adding one, so a run of steps such as
    /// repeated nudges undoes at once. A merged command that
    /// [`Command::is_noop`] reports is dropped, with its entry if nothing else
    /// remains. Merging continues until an undo, redo,
    /// [`mark_saved`](Self::mark_saved) or [`seal`](Self::seal); a plain
    /// [`execute`](Self::execute) can also be extended this way.
    pub fn execute_merging<T: ?Sized>(&mut self, command: C, target: &mut T) -> Result<bool, String>
    where
        C: Command<T>,
    {
        if !command.apply(target)? {
            return Ok(false);
        }
        if self.open
            && let Some(entry) = self.undo.back_mut()
            && let Some(last) = entry.commands.last_mut()
            && last.merge(&command)
        {
            if last.is_noop() {
                entry.commands.pop();
            }
            if entry.commands.is_empty() {
                // The target is back in the previous entry's state.
                self.undo.pop_back();
                self.open = false;
            } else {
                entry.state = self.next_state;
                self.next_state += 1;
            }
            self.redo.clear();
            self.revision += 1;
            return Ok(true);
        }
        self.push(vec![command]);
        Ok(true)
    }

    /// Revert the newest entry, a group's commands in reverse order, and make
    /// it redoable. Returns `Ok(false)` when there is nothing to undo. If a command fails, the entry's commands
    /// already reverted are reapplied, the entry stays, and the error is
    /// returned.
    pub fn undo<T: ?Sized>(&mut self, target: &mut T) -> Result<bool, String>
    where
        C: Command<T>,
    {
        let Some(entry) = self.undo.back() else {
            return Ok(false);
        };
        for (index, command) in entry.commands.iter().enumerate().rev() {
            if let Err(error) = command.revert(target) {
                for command in &entry.commands[index + 1..] {
                    let _ = command.apply(target);
                }
                return Err(error);
            }
        }
        let entry = self.undo.pop_back().expect("the undo entry was reverted");
        self.redo.push(entry);
        self.revision += 1;
        self.open = false;
        Ok(true)
    }

    /// Reapply the newest undone entry, a group's commands in order. Returns
    /// `Ok(false)` when there is nothing to redo. If a command fails, the entry's commands already
    /// reapplied are reverted, the entry stays redoable, and the error is
    /// returned.
    pub fn redo<T: ?Sized>(&mut self, target: &mut T) -> Result<bool, String>
    where
        C: Command<T>,
    {
        let Some(entry) = self.redo.last() else {
            return Ok(false);
        };
        for (index, command) in entry.commands.iter().enumerate() {
            if let Err(error) = command.apply(target) {
                for command in entry.commands[..index].iter().rev() {
                    let _ = command.revert(target);
                }
                return Err(error);
            }
        }
        let entry = self.redo.pop().expect("the redo entry was reapplied");
        self.undo.push_back(entry);
        self.revision += 1;
        self.open = false;
        Ok(true)
    }

    /// Record the current state as saved.
    pub fn mark_saved(&mut self) {
        self.saved = Some(self.current_state());
        self.open = false;
    }

    /// Whether undo and redo have returned to the last save point. A new
    /// history starts saved; a save point that redo can no longer reach is
    /// lost until the next [`mark_saved`](Self::mark_saved).
    pub fn is_saved(&self) -> bool {
        self.saved == Some(self.current_state())
    }

    /// Stop [`execute_merging`](Self::execute_merging) from extending the
    /// newest entry, for example when a text field loses focus.
    pub fn seal(&mut self) {
        self.open = false;
    }

    /// Forget every entry. The current state keeps its saved status.
    pub fn clear(&mut self) {
        self.base = self.current_state();
        self.undo.clear();
        self.redo.clear();
        self.revision += 1;
        self.open = false;
    }

    /// Counts every executed, merged, undone or redone change and every clear,
    /// so an application can tell agents when undo or redo would now do
    /// something different.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// The commands the next undo would revert, in application order.
    pub fn last_undo(&self) -> Option<&[C]> {
        self.undo.back().map(|entry| entry.commands.as_slice())
    }

    /// The commands the next redo would reapply, in application order.
    pub fn last_redo(&self) -> Option<&[C]> {
        self.redo.last().map(|entry| entry.commands.as_slice())
    }

    fn current_state(&self) -> u64 {
        self.undo.back().map_or(self.base, |entry| entry.state)
    }

    fn push(&mut self, commands: Vec<C>) {
        self.redo.clear();
        self.undo.push_back(Entry {
            state: self.next_state,
            commands,
        });
        self.next_state += 1;
        if self.undo.len() > self.capacity
            && let Some(forgotten) = self.undo.pop_front()
        {
            self.base = forgotten.state;
        }
        self.revision += 1;
        self.open = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sets one slot; fails when the slot is missing.
    #[derive(Clone, Debug, PartialEq)]
    struct Set {
        slot: usize,
        before: i32,
        after: i32,
    }

    impl Command<Vec<i32>> for Set {
        fn apply(&self, values: &mut Vec<i32>) -> Result<bool, String> {
            let value = values.get_mut(self.slot).ok_or("missing slot")?;
            if *value == self.after {
                return Ok(false);
            }
            *value = self.after;
            Ok(true)
        }

        fn revert(&self, values: &mut Vec<i32>) -> Result<(), String> {
            *values.get_mut(self.slot).ok_or("missing slot")? = self.before;
            Ok(())
        }

        fn merge(&mut self, next: &Self) -> bool {
            if next.slot != self.slot {
                return false;
            }
            self.after = next.after;
            true
        }

        fn is_noop(&self) -> bool {
            self.before == self.after
        }
    }

    fn set(slot: usize, before: i32, after: i32) -> Set {
        Set {
            slot,
            before,
            after,
        }
    }

    #[test]
    fn execute_undo_redo_and_new_change_clears_redo() {
        let mut values = vec![0, 0];
        let mut history = History::default();
        assert_eq!(history.execute(set(0, 0, 1), &mut values), Ok(true));
        assert_eq!(history.execute(set(1, 0, 2), &mut values), Ok(true));
        assert_eq!(values, [1, 2]);
        assert_eq!(history.undo(&mut values), Ok(true));
        assert_eq!(
            (values.clone(), history.undo_len(), history.redo_len()),
            (vec![1, 0], 1, 1)
        );
        assert_eq!(history.redo(&mut values), Ok(true));
        assert_eq!(values, [1, 2]);
        history.undo(&mut values).unwrap();
        assert_eq!(history.execute(set(1, 0, 5), &mut values), Ok(true));
        assert_eq!(history.redo_len(), 0);
        assert_eq!(history.redo(&mut values), Ok(false));
        assert_eq!(values, [1, 5]);
    }

    #[test]
    fn unchanged_and_failed_commands_record_nothing_and_keep_redo() {
        let mut values = vec![0];
        let mut history = History::default();
        history.execute(set(0, 0, 1), &mut values).unwrap();
        history.undo(&mut values).unwrap();
        let revision = history.revision();
        assert_eq!(history.execute(set(0, 0, 0), &mut values), Ok(false));
        assert!(history.execute(set(3, 0, 1), &mut values).is_err());
        assert_eq!((history.undo_len(), history.redo_len()), (0, 1));
        assert_eq!(history.revision(), revision);
        assert_eq!(history.undo(&mut values), Ok(false));
    }

    #[test]
    fn capacity_forgets_the_oldest_entry() {
        let mut values = vec![0];
        let mut history = History::new(3);
        for value in 1..=5 {
            history
                .execute(set(0, value - 1, value), &mut values)
                .unwrap();
        }
        assert_eq!(history.undo_len(), 3);
        while history.undo(&mut values).unwrap() {}
        assert_eq!(values, [2]);
        assert_eq!(History::<Set>::new(0).capacity(), 1);
    }

    #[test]
    fn a_group_is_one_entry_and_a_failed_group_rolls_back() {
        let mut values = vec![0, 0, 0];
        let mut history = History::default();
        let group = [set(0, 0, 1), set(1, 0, 0), set(2, 0, 3)];
        assert_eq!(history.execute_group(group, &mut values), Ok(true));
        assert_eq!((values.clone(), history.undo_len()), (vec![1, 0, 3], 1));
        assert_eq!(history.last_undo().map(<[Set]>::len), Some(2));
        history.undo(&mut values).unwrap();
        assert_eq!(values, [0, 0, 0]);
        assert_eq!(history.last_redo().map(<[Set]>::len), Some(2));
        history.redo(&mut values).unwrap();
        assert_eq!(values, [1, 0, 3]);

        let failing = [set(0, 1, 7), set(9, 0, 1)];
        assert!(history.execute_group(failing, &mut values).is_err());
        assert_eq!((values.clone(), history.undo_len()), (vec![1, 0, 3], 1));
        assert_eq!(
            history.execute_group([set(0, 1, 1)], &mut values),
            Ok(false)
        );
    }

    #[test]
    fn merging_extends_the_newest_entry_until_sealed_or_undone() {
        let mut values = vec![0, 0];
        let mut history = History::default();
        history.execute_merging(set(0, 0, 1), &mut values).unwrap();
        history.execute_merging(set(0, 1, 2), &mut values).unwrap();
        history.execute_merging(set(0, 2, 3), &mut values).unwrap();
        assert_eq!(history.undo_len(), 1);
        assert_eq!(history.last_undo(), Some(&[set(0, 0, 3)][..]));
        // A command that refuses to merge starts a new entry.
        history.execute_merging(set(1, 0, 1), &mut values).unwrap();
        assert_eq!(history.undo_len(), 2);
        history.seal();
        history.execute_merging(set(1, 1, 2), &mut values).unwrap();
        assert_eq!(history.undo_len(), 3);
        history.undo(&mut values).unwrap();
        history.execute_merging(set(1, 1, 4), &mut values).unwrap();
        assert_eq!(history.undo_len(), 3);
        history.undo(&mut values).unwrap();
        history.undo(&mut values).unwrap();
        history.undo(&mut values).unwrap();
        assert_eq!(values, [0, 0]);
    }

    #[test]
    fn a_merged_run_that_returns_to_its_start_leaves_no_entry() {
        let mut values = vec![0];
        let mut history = History::default();
        history.execute(set(0, 0, 5), &mut values).unwrap();
        history.mark_saved();
        history.execute_merging(set(0, 5, 6), &mut values).unwrap();
        assert!(!history.is_saved());
        history.execute_merging(set(0, 6, 5), &mut values).unwrap();
        assert_eq!((values.clone(), history.undo_len()), (vec![5], 1));
        assert!(history.is_saved(), "back in the saved state");
        // The remaining entry was sealed before the run and stays separate.
        history.execute_merging(set(0, 5, 7), &mut values).unwrap();
        assert_eq!(history.undo_len(), 2);
    }

    #[test]
    fn merging_stops_at_a_save_point_and_failures_leave_the_entry() {
        let mut values = vec![0];
        let mut history = History::default();
        history.execute_merging(set(0, 0, 1), &mut values).unwrap();
        history.mark_saved();
        history.execute_merging(set(0, 1, 2), &mut values).unwrap();
        assert_eq!(history.undo_len(), 2);
        assert!(history.execute_merging(set(4, 0, 1), &mut values).is_err());
        assert_eq!(history.last_undo(), Some(&[set(0, 1, 2)][..]));
        history.undo(&mut values).unwrap();
        assert!(history.is_saved());
        let revision = history.revision();
        history.clear();
        assert!(history.revision() > revision);
    }

    #[test]
    fn save_point_follows_undo_redo_merges_and_lost_branches() {
        let mut values = vec![0];
        let mut history = History::new(2);
        assert!(history.is_saved());
        history.execute(set(0, 0, 1), &mut values).unwrap();
        assert!(!history.is_saved());
        history.mark_saved();
        history.execute(set(0, 1, 2), &mut values).unwrap();
        assert!(!history.is_saved());
        history.undo(&mut values).unwrap();
        assert!(history.is_saved());
        history.redo(&mut values).unwrap();
        assert!(!history.is_saved());
        history.undo(&mut values).unwrap();
        history.undo(&mut values).unwrap();
        assert!(!history.is_saved());
        // Redo can still reach the save point.
        history.redo(&mut values).unwrap();
        assert!(history.is_saved());
        // A merged run is one entry.
        history.execute_merging(set(0, 1, 2), &mut values).unwrap();
        history.execute_merging(set(0, 2, 3), &mut values).unwrap();
        assert_eq!(history.undo_len(), 2);
        assert!(!history.is_saved());
        history.undo(&mut values).unwrap();
        assert!(history.is_saved());
        // A new branch after undoing past the save point loses it.
        history.undo(&mut values).unwrap();
        history.execute(set(0, 0, 9), &mut values).unwrap();
        assert!(!history.is_saved());
        while history.undo(&mut values).unwrap() {}
        assert_eq!(values, [0]);
        assert!(!history.is_saved());
        // Clearing keeps the current state's saved status.
        history.mark_saved();
        history.clear();
        assert!(history.is_saved() && !history.can_undo() && !history.can_redo());
    }

    #[test]
    fn eviction_keeps_the_save_point_reachable_only_while_retained() {
        let mut values = vec![0];
        let mut history = History::new(2);
        history.execute(set(0, 0, 1), &mut values).unwrap();
        history.execute(set(0, 1, 2), &mut values).unwrap();
        history.undo(&mut values).unwrap();
        history.undo(&mut values).unwrap();
        assert!(history.is_saved());
        history.redo(&mut values).unwrap();
        history.redo(&mut values).unwrap();
        history.execute(set(0, 2, 3), &mut values).unwrap();
        // The entry leaving the saved state was forgotten.
        while history.undo(&mut values).unwrap() {}
        assert_eq!(values, [1]);
        assert!(!history.is_saved());
    }

    #[test]
    fn a_failed_undo_or_redo_leaves_target_and_entry_in_place() {
        let mut values = vec![0, 0];
        let mut history = History::default();
        history
            .execute_group([set(0, 0, 1), set(1, 0, 2)], &mut values)
            .unwrap();
        values.truncate(1);
        assert!(history.undo(&mut values).is_err());
        assert_eq!((values.clone(), history.undo_len()), (vec![1], 1));
        values.push(2);
        history.undo(&mut values).unwrap();
        values.truncate(1);
        assert!(history.redo(&mut values).is_err());
        assert_eq!((values.clone(), history.redo_len()), (vec![0], 1));
    }
}
