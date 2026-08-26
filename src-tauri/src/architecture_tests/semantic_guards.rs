use std::path::Path;

use super::model::{CanonicalDependency, CanonicalOrigin, RustLayer};

const GRAPH_DOCUMENT_FILES: [&str; 4] = [
    "src-tauri/src/graph_document/mod.rs",
    "src-tauri/src/graph_document/identity.rs",
    "src-tauri/src/graph_document/model.rs",
    "src-tauri/src/graph_document/resource_path.rs",
];

pub(super) fn graph_document_json_violations(repository_root: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    for relative in GRAPH_DOCUMENT_FILES {
        let source = std::fs::read_to_string(repository_root.join(relative))
            .unwrap_or_else(|error| panic!("failed to read {relative}: {error}"));
        let production = source.split("#[cfg(test)]").next().unwrap_or(&source);
        let uses = production.match_indices("serde_json").collect::<Vec<_>>();
        if relative == "src-tauri/src/graph_document/model.rs" {
            let exact_alias = "pub type TypedValue = serde_json::Value;";
            if uses.len() != 1 || !production.contains(exact_alias) {
                violations.push(format!(
                    "{relative}: expected exactly the persisted TypedValue serde_json alias"
                ));
            }
        } else if !uses.is_empty() {
            violations.push(format!("{relative}: serde_json is not allowed"));
        }
    }
    violations
}

pub(super) fn project_to_graph_production_edges(
    dependencies: &[CanonicalDependency],
    classification: &std::collections::BTreeMap<String, RustLayer>,
) -> Vec<String> {
    dependencies
        .iter()
        .filter(|dependency| {
            classification.get(&dependency.source_file) == Some(&RustLayer::Project)
        })
        .filter_map(|dependency| match &dependency.origin {
            CanonicalOrigin::Repository {
                repository_relative_declaration_file,
                fully_qualified_target,
                ..
            } if repository_relative_declaration_file.starts_with("src-tauri/src/graph/") => Some(
                format!("{} -> {fully_qualified_target}", dependency.source_file),
            ),
            _ => None,
        })
        .collect()
}
