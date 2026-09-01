//! Canonical on-disk layout for a YssBI project.
//!
//! This crate owns names and path classification only. Project I/O, watcher
//! delivery, and document schemas remain in their respective owners.

use std::path::{Component, Path};

pub const PROJECT_METADATA_FILE: &str = "metadata.yssbi";
pub const GLOBAL_VARIABLES_FILE: &str = "variables.yssbi-vars";

pub const EVENTS_DIR: &str = "events";
pub const EVENT_EXTENSION: &str = "yssbi-event";

pub const FUNCTIONS_DIR: &str = "functions";
pub const FUNCTION_EXTENSION: &str = "yssbi-function";

pub const CHARTS_DIR: &str = "charts";
pub const CHART_EXTENSION: &str = "yssbi-chart";

pub const DATABASE_DIR: &str = "database";
pub const PROJECT_DUCKDB_FILE: &str = "project.duckdb";

pub const PROJECT_CONTENT_DIRECTORIES: [&str; 4] =
    [EVENTS_DIR, FUNCTIONS_DIR, CHARTS_DIR, DATABASE_DIR];

/// Returns whether a safe project-relative path can change the project index.
pub fn is_project_index_input_path(path: &Path) -> bool {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return false;
    }

    let normalized = path.to_string_lossy().replace('\\', "/");
    normalized == PROJECT_METADATA_FILE
        || normalized == GLOBAL_VARIABLES_FILE
        || [EVENTS_DIR, FUNCTIONS_DIR, CHARTS_DIR, DATABASE_DIR]
            .into_iter()
            .any(|directory| is_descendant(&normalized, directory))
}

fn is_descendant(path: &str, directory: &str) -> bool {
    path.strip_prefix(directory)
        .and_then(|suffix| suffix.strip_prefix('/'))
        .is_some_and(|suffix| !suffix.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_layout_names_remain_stable() {
        assert_eq!(PROJECT_METADATA_FILE, "metadata.yssbi");
        assert_eq!(GLOBAL_VARIABLES_FILE, "variables.yssbi-vars");
        assert_eq!(EVENTS_DIR, "events");
        assert_eq!(EVENT_EXTENSION, "yssbi-event");
        assert_eq!(FUNCTIONS_DIR, "functions");
        assert_eq!(FUNCTION_EXTENSION, "yssbi-function");
        assert_eq!(CHARTS_DIR, "charts");
        assert_eq!(CHART_EXTENSION, "yssbi-chart");
        assert_eq!(DATABASE_DIR, "database");
        assert_eq!(PROJECT_DUCKDB_FILE, "project.duckdb");
    }

    #[test]
    fn project_index_inputs_cover_documents_and_content_directories() {
        for path in [
            PROJECT_METADATA_FILE,
            GLOBAL_VARIABLES_FILE,
            "events/Main.yssbi-event",
            r"events\Main.yssbi-event",
            "functions/Mean.yssbi-function",
            "charts/Sales.yssbi-chart",
            "database/project.duckdb",
        ] {
            assert!(is_project_index_input_path(Path::new(path)), "{path}");
        }

        for path in [
            "",
            "README.md",
            "events",
            "database",
            "../metadata.yssbi",
            "events/../metadata.yssbi",
            "/metadata.yssbi",
            r"C:\metadata.yssbi",
        ] {
            assert!(!is_project_index_input_path(Path::new(path)), "{path}");
        }

        let absolute = std::env::current_dir().unwrap().join(PROJECT_METADATA_FILE);
        assert!(!is_project_index_input_path(&absolute));
    }
}
