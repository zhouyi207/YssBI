//! Database edit operations, undo/redo history, and edit-state projection.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq)]
pub enum EditOperation {
    EditCell {
        row: usize,
        row_id: Option<i64>,
        col: String,
        old_value: serde_json::Value,
        new_value: serde_json::Value,
    },
    AddRow {
        index: usize,
        row_id: Option<i64>,
    },
    DeleteRow {
        index: usize,
        row_id: Option<i64>,
        data: Vec<serde_json::Value>,
    },
    AddColumn {
        name: String,
        dtype: String,
    },
    DeleteColumn {
        name: String,
        dtype: String,
        row_ids: Vec<i64>,
        row_fingerprints: Vec<u64>,
        data: Vec<serde_json::Value>,
    },
    RenameColumn {
        old_name: String,
        new_name: String,
    },
    CastColumn {
        col: String,
        old_data: Vec<serde_json::Value>,
        old_dtype: String,
        new_dtype: String,
    },
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditState {
    pub can_undo: bool,
    pub can_redo: bool,
    pub is_modified: bool,
    pub undo_count: usize,
    pub redo_count: usize,
}

#[derive(Debug, Clone, Default)]
pub struct EditHistory {
    undo_stack: Vec<EditOperation>,
    redo_stack: Vec<EditOperation>,
}

impl EditHistory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, op: EditOperation) {
        self.undo_stack.push(op);
        self.redo_stack.clear();
    }

    pub fn pop_undo(&mut self) -> Option<EditOperation> {
        self.undo_stack.pop()
    }

    pub fn push_redo(&mut self, op: EditOperation) {
        self.redo_stack.push(op);
    }

    pub fn pop_redo(&mut self) -> Option<EditOperation> {
        self.redo_stack.pop()
    }

    pub fn push_undo(&mut self, op: EditOperation) {
        self.undo_stack.push(op);
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    pub fn state(&self) -> EditState {
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
    use serde_json::json;

    fn add_row(index: usize) -> EditOperation {
        EditOperation::AddRow {
            index,
            row_id: None,
        }
    }

    #[test]
    fn new_edit_after_undo_clears_redo_history() {
        let mut history = EditHistory::new();
        history.push(add_row(0));
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

        history.push(add_row(1));

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

    #[test]
    fn edit_state_wire_is_strict_camel_case() {
        let value = serde_json::to_value(EditState {
            can_undo: true,
            can_redo: false,
            is_modified: true,
            undo_count: 2,
            redo_count: 0,
        })
        .expect("serialize edit state");

        assert_eq!(value["canUndo"], json!(true));
        assert_eq!(value["canRedo"], json!(false));
        assert_eq!(value["isModified"], json!(true));
        assert_eq!(value["undoCount"], json!(2));
        assert_eq!(value["redoCount"], json!(0));
        assert!(value.get("can_undo").is_none());
    }
}
