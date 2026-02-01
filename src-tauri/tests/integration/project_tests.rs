//! Project 序列化/反序列化测试

use yssbi_lib::project::ProjectData;

#[test]
fn test_serialize_project() {
    let project = ProjectData::new();
    let json = project.to_json().unwrap();
    assert!(json.contains("globalVariables"));
    assert!(json.contains("events"));
}

#[test]
fn test_deserialize_project() {
    let json = r#"{
        "globalVariables": {},
        "events": {},
        "functions": {},
        "macros": {},
        "metadata": {
            "exportTime": "2024-01-01T00:00:00Z",
            "appVersion": "0.1.0"
        }
    }"#;
    let project = ProjectData::from_json(json).unwrap();
    assert!(project.events.is_empty());
}