use serde::Serialize;

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditState {
    pub can_undo: bool,
    pub can_redo: bool,
    pub is_modified: bool,
    pub undo_count: usize,
    pub redo_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
