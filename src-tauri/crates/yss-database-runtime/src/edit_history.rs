//! Runtime-owned undo/redo records.
use yss_database_contract::EditState;

#[derive(Debug, Clone)]
pub(crate) struct EditHistory<T> {
    undo_stack: Vec<T>,
    redo_stack: Vec<T>,
}

impl<T> Default for EditHistory<T> {
    fn default() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }
}

impl<T> EditHistory<T> {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn push(&mut self, op: T) {
        self.undo_stack.push(op);
        self.redo_stack.clear();
    }

    pub(crate) fn pop_undo(&mut self) -> Option<T> {
        self.undo_stack.pop()
    }

    pub(crate) fn push_redo(&mut self, op: T) {
        self.redo_stack.push(op);
    }

    pub(crate) fn pop_redo(&mut self) -> Option<T> {
        self.redo_stack.pop()
    }

    pub(crate) fn push_undo(&mut self, op: T) {
        self.undo_stack.push(op);
    }

    pub(crate) fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    pub(crate) fn state(&self) -> EditState {
        EditState {
            can_undo: !self.undo_stack.is_empty(),
            can_redo: !self.redo_stack.is_empty(),
            is_modified: !self.undo_stack.is_empty(),
            undo_count: self.undo_stack.len(),
            redo_count: self.redo_stack.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_edit_after_undo_clears_redo_history() {
        let mut history = EditHistory::new();
        history.push(0usize);
        let operation = history.pop_undo().expect("undo operation");
        history.push_redo(operation);
        assert_eq!(
            history.state(),
            EditState {
                can_undo: false,
                can_redo: true,
                is_modified: false,
                undo_count: 0,
                redo_count: 1,
            }
        );

        history.push(1usize);

        assert_eq!(
            history.state(),
            EditState {
                can_undo: true,
                can_redo: false,
                is_modified: true,
                undo_count: 1,
                redo_count: 0,
            }
        );
    }
}
