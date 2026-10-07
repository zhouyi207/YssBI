//! Canonical on-disk layout for a YssBI project.
//!
//! This crate owns names and path classification only. Project I/O, watcher
//! delivery, and document schemas remain in their respective owners.

use std::ffi::OsStr;
use std::path::{Component, Path};

pub const PROJECT_METADATA_FILE: &str = "metadata.yssbi";

pub const EVENT_GRAPHS_DIR: &str = "events";
pub const EVENT_GRAPH_EXTENSION: &str = "yssbi-event";

pub const FUNCTION_GRAPHS_DIR: &str = "functions";
pub const FUNCTION_GRAPH_EXTENSION: &str = "yssbi-function";

pub const CHARTS_DIR: &str = "charts";
pub const CHART_EXTENSION: &str = "yssbi-chart";

pub const MINDS_DIR: &str = "minds";
pub const MIND_EXTENSION: &str = "yssbi-mind";

pub const DOCS_DIR: &str = "docs";
pub const DOC_EXTENSION: &str = "md";

pub const DATABASE_DIR: &str = "database";
pub const PROJECT_DATASET_CATALOG_FILE: &str = "catalog.sqlite";

pub const PROJECT_CONTENT_DIRECTORIES: [&str; 6] = [
    EVENT_GRAPHS_DIR,
    FUNCTION_GRAPHS_DIR,
    CHARTS_DIR,
    DATABASE_DIR,
    MINDS_DIR,
    DOCS_DIR,
];

/// Returns whether a safe project-relative path can change the project index.
pub fn is_project_index_input_path(path: &Path) -> bool {
    let mut components = path.components();
    let Some(Component::Normal(first)) = components.next() else {
        return false;
    };
    if components
        .clone()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return false;
    }

    if first == OsStr::new(PROJECT_METADATA_FILE) {
        return components.next().is_none();
    }

    PROJECT_CONTENT_DIRECTORIES
        .into_iter()
        .any(|directory| first == OsStr::new(directory))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_index_inputs_cover_documents_and_content_directories() {
        for path in [
            PROJECT_METADATA_FILE,
            EVENT_GRAPHS_DIR,
            FUNCTION_GRAPHS_DIR,
            CHARTS_DIR,
            DATABASE_DIR,
            "events/Main.yssbi-event",
            "functions/Mean.yssbi-function",
            "charts/Sales.yssbi-chart",
            "database/catalog.sqlite",
        ] {
            assert!(is_project_index_input_path(Path::new(path)), "{path}");
        }
        let native = Path::new(EVENT_GRAPHS_DIR).join("Main.yssbi-event");
        assert!(is_project_index_input_path(&native));

        for path in [
            "",
            "README.md",
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

    #[cfg(unix)]
    #[test]
    fn unix_backslash_names_do_not_count_as_project_directories() {
        for path in [
            r"events\Main.yssbi-event",
            r"events\..\README.md",
            r"events\folder/Main.yssbi-event",
        ] {
            assert!(!is_project_index_input_path(Path::new(path)), "{path}");
        }
    }
}
