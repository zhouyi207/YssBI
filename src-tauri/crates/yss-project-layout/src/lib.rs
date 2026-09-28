//! Canonical on-disk layout for a YssBI project.
//!
//! This crate owns names and path classification only. Project I/O, watcher
//! delivery, and document schemas remain in their respective owners.

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
        || PROJECT_CONTENT_DIRECTORIES
            .into_iter()
            .any(|directory| normalized == directory || is_descendant(&normalized, directory))
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
    fn project_index_inputs_cover_documents_and_content_directories() {
        for path in [
            PROJECT_METADATA_FILE,
            EVENT_GRAPHS_DIR,
            FUNCTION_GRAPHS_DIR,
            CHARTS_DIR,
            DATABASE_DIR,
            "events/Main.yssbi-event",
            r"events\Main.yssbi-event",
            "functions/Mean.yssbi-function",
            "charts/Sales.yssbi-chart",
            "database/catalog.sqlite",
        ] {
            assert!(is_project_index_input_path(Path::new(path)), "{path}");
        }

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
}
