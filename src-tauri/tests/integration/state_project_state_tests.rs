//! ProjectState 基础功能测试

use yssbi_lib::state::ProjectState;

#[test]
fn test_project_state_new() {
    let state = ProjectState::new();
    let data = state.get_data();
    assert!(data.events.is_empty());
    assert!(data.functions.is_empty());
    assert!(data.macros.is_empty());
    assert!(data.global_variables.is_empty());
}