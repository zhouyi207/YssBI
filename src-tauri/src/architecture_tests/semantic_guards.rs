use std::collections::BTreeMap;
use std::path::Path;

use super::model::{CanonicalDependency, CanonicalOrigin, RustDependencyKind, RustLayer};

pub(super) const PURE_LEAF_GRAPH_DOCUMENT_JSON_RULE: &str = "rust.pure-leaf.graph-document-json";
const GRAPH_DOCUMENT_MODEL: &str = "src-tauri/src/graph_document/model.rs";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SemanticGuardViolation {
    pub(super) rule_id: &'static str,
    pub(super) source_file: String,
    pub(super) reason: SemanticGuardViolationReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SemanticGuardViolationReason {
    UnexpectedSerdeJsonDependency {
        dependency_kind: RustDependencyKind,
        canonical_origin_target: String,
    },
    MissingExactTypedValueAlias,
    InvalidTypedValueAlias,
    ModelSourceUnreadable,
    ModelSourceUnparseable,
}

pub(super) fn pure_leaf_graph_document_json_violations(
    repository_root: &Path,
    dependencies: &[CanonicalDependency],
    classification: &BTreeMap<String, RustLayer>,
) -> Vec<SemanticGuardViolation> {
    let mut violations = Vec::new();
    let mut allowed_dependencies = 0;
    for dependency in dependencies.iter().filter(|dependency| {
        classification.get(&dependency.source_file) == Some(&RustLayer::PureLeaf)
            && matches!(
                &dependency.origin,
                CanonicalOrigin::External(origin) if origin.package_name == "serde_json"
            )
    }) {
        let allowed = dependency.source_file == GRAPH_DOCUMENT_MODEL
            && dependency.kind == RustDependencyKind::Path
            && dependency.canonical_origin_target == "external:serde_json::Value";
        if allowed {
            allowed_dependencies += 1;
        } else {
            violations.push(SemanticGuardViolation {
                rule_id: PURE_LEAF_GRAPH_DOCUMENT_JSON_RULE,
                source_file: dependency.source_file.clone(),
                reason: SemanticGuardViolationReason::UnexpectedSerdeJsonDependency {
                    dependency_kind: dependency.kind,
                    canonical_origin_target: dependency.canonical_origin_target.clone(),
                },
            });
        }
    }

    let source = match std::fs::read_to_string(repository_root.join(GRAPH_DOCUMENT_MODEL)) {
        Ok(source) => source,
        Err(_) => {
            violations.push(SemanticGuardViolation {
                rule_id: PURE_LEAF_GRAPH_DOCUMENT_JSON_RULE,
                source_file: GRAPH_DOCUMENT_MODEL.to_owned(),
                reason: SemanticGuardViolationReason::ModelSourceUnreadable,
            });
            return violations;
        }
    };
    let syntax = match syn::parse_file(&source) {
        Ok(syntax) => syntax,
        Err(_) => {
            violations.push(SemanticGuardViolation {
                rule_id: PURE_LEAF_GRAPH_DOCUMENT_JSON_RULE,
                source_file: GRAPH_DOCUMENT_MODEL.to_owned(),
                reason: SemanticGuardViolationReason::ModelSourceUnparseable,
            });
            return violations;
        }
    };
    let typed_value_aliases = syntax
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Type(alias) if alias.ident == "TypedValue" => Some(alias),
            _ => None,
        })
        .collect::<Vec<_>>();
    if typed_value_aliases.is_empty() || allowed_dependencies != 1 {
        violations.push(SemanticGuardViolation {
            rule_id: PURE_LEAF_GRAPH_DOCUMENT_JSON_RULE,
            source_file: GRAPH_DOCUMENT_MODEL.to_owned(),
            reason: SemanticGuardViolationReason::MissingExactTypedValueAlias,
        });
    } else if typed_value_aliases.len() != 1 || !exact_typed_value_alias(typed_value_aliases[0]) {
        violations.push(SemanticGuardViolation {
            rule_id: PURE_LEAF_GRAPH_DOCUMENT_JSON_RULE,
            source_file: GRAPH_DOCUMENT_MODEL.to_owned(),
            reason: SemanticGuardViolationReason::InvalidTypedValueAlias,
        });
    }
    violations
}

fn exact_typed_value_alias(alias: &syn::ItemType) -> bool {
    if !matches!(alias.vis, syn::Visibility::Public(_)) || !alias.generics.params.is_empty() {
        return false;
    }
    let syn::Type::Path(path) = alias.ty.as_ref() else {
        return false;
    };
    path.qself.is_none()
        && path.path.leading_colon.is_none()
        && path.path.segments.len() == 2
        && path.path.segments[0].ident == "serde_json"
        && path.path.segments[1].ident == "Value"
        && path
            .path
            .segments
            .iter()
            .all(|segment| matches!(segment.arguments, syn::PathArguments::None))
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

pub(super) fn graph_project_revision_bridge_violations(repository_root: &Path) -> Vec<String> {
    let identity_path = "src-tauri/src/project/identity.rs";
    let identity = std::fs::read_to_string(repository_root.join(identity_path))
        .unwrap_or_else(|error| panic!("failed to read {identity_path}: {error}"));
    let mutation_path = "src-tauri/src/node_system/document/mutation.rs";
    let mutation = std::fs::read_to_string(repository_root.join(mutation_path))
        .unwrap_or_else(|error| panic!("failed to read {mutation_path}: {error}"));
    let mut violations = Vec::new();
    for forbidden in [
        "impl From<crate::graph_document::GraphRevision> for ResourceRevision",
        "impl From<ResourceRevision> for crate::graph_document::GraphRevision",
        "impl PartialEq<crate::graph_document::GraphRevision> for ResourceRevision",
        "impl PartialEq<ResourceRevision> for crate::graph_document::GraphRevision",
    ] {
        if identity.contains(forbidden) {
            violations.push(format!(
                "{identity_path}: implicit revision bridge `{forbidden}`"
            ));
        }
    }
    if mutation.contains("base_revision: impl Into<ResourceRevision>") {
        violations.push(format!(
            "{mutation_path}: MutationRequest::new accepts an implicit revision bridge"
        ));
    }
    violations
}
