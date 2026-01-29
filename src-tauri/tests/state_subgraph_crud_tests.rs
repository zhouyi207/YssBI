//! SubGraph CRUD 操作测试

use yssbi_lib::state::ProjectState;
use yssbi_lib::project::{SubGraphData, SubGraphType, CanvasState};
use std::collections::HashMap;

#[test]
fn test_event_crud() {
    let state = ProjectState::new();
    let event_data = SubGraphData {
        id: "test-event".to_string(),
        name: "Test Event".to_string(),
        sub_type: SubGraphType::Event,
        nodes: vec![],
        canvas: CanvasState::default(),
        variables: HashMap::new(),
        inputs: vec![],
        outputs: vec![],
    };

    // Create
    state
        .create_event("test-event".to_string(), event_data.clone())
        .unwrap();
    assert_eq!(state.get_events().len(), 1);

    // Read
    let retrieved = state.get_event("test-event").unwrap();
    assert_eq!(retrieved.name, "Test Event");

    // Update
    let mut updated = event_data.clone();
    updated.name = "Updated Event".to_string();
    state.update_event("test-event", updated).unwrap();
    assert_eq!(state.get_event("test-event").unwrap().name, "Updated Event");

    // Delete
    state.delete_event("test-event").unwrap();
    assert!(state.get_events().is_empty());
}