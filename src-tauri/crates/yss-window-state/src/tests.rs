use super::{WindowKind, WindowState, WindowStateError, WindowStateStore};
use std::fs;
use std::path::{Path, PathBuf};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!("yssbi-{label}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("failed to clean up window state test directory: {error}");
        }
    }
}

#[test]
fn kind_contract_preserves_wire_storage_defaults_and_load() {
    let expected = [
        (WindowKind::Main, "main", 1600, 900),
        (WindowKind::DatabaseEditor, "databaseEditor", 1000, 600),
        (WindowKind::SourceInspector, "sourceInspector", 1000, 600),
        (WindowKind::Logs, "logs", 1000, 600),
        (WindowKind::Plot, "plot", 960, 800),
        (WindowKind::Info, "info", 960, 800),
        (WindowKind::Bayes, "bayes", 960, 800),
    ];
    assert_eq!(
        WindowKind::all().collect::<Vec<_>>(),
        expected
            .iter()
            .map(|(kind, _, _, _)| *kind)
            .collect::<Vec<_>>(),
    );

    let temporary = TestDirectory::new("window-state-kind-contract");
    let file_path = temporary.path().join("window_state.json");
    let store = WindowStateStore::load(file_path.clone());

    for (kind, key, width, height) in expected {
        assert_eq!(kind.as_str(), key);
        assert_eq!(serde_json::to_value(kind).unwrap(), serde_json::json!(key));
        let state = store.get(kind);
        assert_eq!(
            (
                state.width,
                state.height,
                state.x,
                state.y,
                state.is_maximized,
            ),
            (width, height, None, None, false),
        );
    }

    store
        .set(
            WindowKind::SourceInspector,
            WindowState {
                width: 1111,
                height: 777,
                x: Some(-20),
                y: Some(45),
                is_maximized: true,
            },
        )
        .unwrap();
    let stored: serde_json::Value = serde_json::from_slice(&fs::read(&file_path).unwrap()).unwrap();
    assert_eq!(
        stored,
        serde_json::json!({
            "main": null,
            "databaseEditor": null,
            "sourceInspector": {
                "width": 1111,
                "height": 777,
                "x": -20,
                "y": 45,
                "isMaximized": true
            },
            "logs": null,
            "plot": null,
            "info": null,
            "bayes": null
        }),
    );

    let loaded = WindowStateStore::load(file_path);
    let state = loaded.get(WindowKind::SourceInspector);
    assert_eq!(
        (
            state.width,
            state.height,
            state.x,
            state.y,
            state.is_maximized,
        ),
        (1111, 777, Some(-20), Some(45), true),
    );
}

#[test]
fn set_failure_preserves_last_committed_state() {
    let temporary = TestDirectory::new("window-state-set-failure");
    let file_path = temporary.path().join("window_state.json");
    let store = WindowStateStore::load(file_path.clone());
    let committed = WindowState {
        width: 1200,
        height: 700,
        x: Some(30),
        y: Some(40),
        is_maximized: true,
    };
    store.set(WindowKind::Main, committed).unwrap();

    fs::remove_file(&file_path).unwrap();
    fs::create_dir(&file_path).unwrap();
    let result = store.set(
        WindowKind::Main,
        WindowState {
            width: 800,
            height: 500,
            x: Some(10),
            y: Some(20),
            is_maximized: false,
        },
    );

    assert!(matches!(result, Err(WindowStateError::Persist(_))));
    let observed = store.get(WindowKind::Main);
    assert_eq!(
        (
            observed.width,
            observed.height,
            observed.x,
            observed.y,
            observed.is_maximized,
        ),
        (1200, 700, Some(30), Some(40), true),
    );
}
