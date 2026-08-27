use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::dependency_audit::collect_production_modules;
use super::model::{
    ArchitectureAuditError, CanonicalDependency, CanonicalOrigin, RustDependencyKind, RustLayer,
    RustModule,
};
use super::policy::classify_rust_sources;
use crate::test_support::source_audit::is_test_only;
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{Expr, ImplItem, Item, Token, TraitItem, Type, UseTree, Visibility};

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

const WORKER_FILE: &str = "src-tauri/src/sci/api/bayes/worker.rs";
const JULIA_WORKER_ADAPTER_FILES: &[&str] = &[
    "src-tauri/src/julia/bayes_worker_adapter/mod.rs",
    "src-tauri/src/julia/bayes_worker_adapter/fit.rs",
    "src-tauri/src/julia/bayes_worker_adapter/predictor.rs",
];
const SCIENTIFIC_BOUNDARY_FILES: &[&str] = &[
    "src-tauri/src/execution/ports/scientific.rs",
    "src-tauri/src/backend_adapters/mod.rs",
    "src-tauri/src/backend_adapters/execution/mod.rs",
    "src-tauri/src/backend_adapters/execution/scientific.rs",
];
const SCIENTIFIC_PORT_FILE: &str = "src-tauri/src/execution/ports/scientific.rs";
const SCIENTIFIC_ADAPTER_FILE: &str = "src-tauri/src/backend_adapters/execution/scientific.rs";
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct WorkerFunction {
    owner: &'static str,
    method: &'static str,
}

const AUTHORITY_FUNCTIONS: &[WorkerFunction] = &[
    WorkerFunction {
        owner: "BayesTaskHandle",
        method: "issue_for_worker",
    },
    WorkerFunction {
        owner: "BayesArtifactHandle",
        method: "mint_for_worker",
    },
    WorkerFunction {
        owner: "BayesTaskResult",
        method: "validated_worker_result",
    },
    WorkerFunction {
        owner: "BayesArtifact",
        method: "from_worker",
    },
    WorkerFunction {
        owner: "BayesInferenceSnapshot",
        method: "from_worker",
    },
];
const ALLOWED_WORKER_FUNCTIONS: &[WorkerFunction] = &[
    WorkerFunction {
        owner: "ArtifactId",
        method: "try_from",
    },
    WorkerFunction {
        owner: "ValidatedBayesTask",
        method: "try_new",
    },
    WorkerFunction {
        owner: "ValidatedBayesTask",
        method: "task_id",
    },
    WorkerFunction {
        owner: "ValidatedBayesTask",
        method: "model",
    },
    WorkerFunction {
        owner: "ValidatedBayesTask",
        method: "inputs",
    },
    WorkerFunction {
        owner: "BayesTaskHandle",
        method: "issue_for_worker",
    },
    WorkerFunction {
        owner: "BayesTaskHandle",
        method: "task_id",
    },
    WorkerFunction {
        owner: "BayesTaskHandle",
        method: "generation",
    },
    WorkerFunction {
        owner: "BayesArtifactHandle",
        method: "mint_for_worker",
    },
    WorkerFunction {
        owner: "BayesArtifactHandle",
        method: "task",
    },
    WorkerFunction {
        owner: "BayesArtifactHandle",
        method: "artifact_id",
    },
    WorkerFunction {
        owner: "BayesArtifact",
        method: "from_worker",
    },
    WorkerFunction {
        owner: "BayesArtifact",
        method: "handle",
    },
    WorkerFunction {
        owner: "BayesArtifact",
        method: "media_type",
    },
    WorkerFunction {
        owner: "BayesArtifact",
        method: "bytes",
    },
    WorkerFunction {
        owner: "BayesInferenceSnapshot",
        method: "from_worker",
    },
    WorkerFunction {
        owner: "BayesInferenceSnapshot",
        method: "task",
    },
    WorkerFunction {
        owner: "BayesInferenceSnapshot",
        method: "summaries",
    },
    WorkerFunction {
        owner: "BayesInferenceSnapshot",
        method: "diagnostics",
    },
    WorkerFunction {
        owner: "BayesTaskResult",
        method: "validated_worker_result",
    },
    WorkerFunction {
        owner: "BayesTaskResult",
        method: "task",
    },
    WorkerFunction {
        owner: "BayesTaskResult",
        method: "inference",
    },
    WorkerFunction {
        owner: "BayesTaskResult",
        method: "artifacts",
    },
];
const PUBLIC_ASSOCIATED_FUNCTIONS: &[WorkerFunction] = &[WorkerFunction {
    owner: "ValidatedBayesTask",
    method: "try_new",
}];
const SEALED_AUTHORITY_TYPES: &[&str] = &[
    "BayesTaskHandle",
    "BayesArtifactHandle",
    "BayesTaskResult",
    "BayesArtifact",
    "BayesInferenceSnapshot",
];
const WORKER_SURFACE_TYPES: &[&str] = &[
    "BayesTaskId",
    "ArtifactId",
    "ValidatedBayesTask",
    "BayesTaskHandle",
    "BayesArtifactHandle",
    "BayesTaskResult",
    "BayesArtifact",
    "BayesInferenceSnapshot",
];
const PRIVATE_FIELD_TYPES: &[&str] = &[
    "BayesTaskId",
    "ArtifactId",
    "BayesTaskGeneration",
    "BayesTaskHandle",
    "ValidatedBayesTask",
    "BayesTaskResult",
    "BayesArtifactHandle",
    "BayesArtifact",
    "BayesInferenceSnapshot",
];
const WORKER_PRIVATE_FIELDS: &[&str] = &[
    "task_id",
    "generation",
    "task",
    "artifact",
    "model",
    "inputs",
    "inference",
    "artifacts",
    "handle",
    "media_type",
    "bytes",
];

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct BayesWorkerAuthorityViolation {
    source_file: String,
    kind: &'static str,
    target: String,
}

fn bayes_worker_authority_violations(
    repository_root: &Path,
    modules: &[RustModule],
    classification: &BTreeMap<String, RustLayer>,
) -> Result<Vec<BayesWorkerAuthorityViolation>, ArchitectureAuditError> {
    let source_files = modules
        .iter()
        .map(|module| module.repository_relative_source_file.as_str())
        .collect::<BTreeSet<_>>();
    let mut violations = Vec::new();
    for source_file in source_files {
        let Some(layer) = classification.get(source_file).copied() else {
            continue;
        };
        let path = repository_root.join(source_file);
        let source =
            std::fs::read_to_string(&path).map_err(|source| ArchitectureAuditError::Io {
                path: path.clone(),
                source,
            })?;
        let mut source_violations = bayes_worker_source_violations(source_file, layer, &source)
            .map_err(|source| ArchitectureAuditError::SourceParse {
                path: path.clone(),
                source,
            })?;
        violations.append(&mut source_violations);
    }
    violations.sort();
    Ok(violations)
}

fn bayes_worker_source_violations(
    source_file: &str,
    layer: RustLayer,
    source: &str,
) -> Result<Vec<BayesWorkerAuthorityViolation>, syn::Error> {
    let syntax = syn::parse_file(source)?;
    let root_scope = build_worker_scope(source_module_path(source_file), &syntax.items, &[]);
    let allow_authority =
        source_file == WORKER_FILE || JULIA_WORKER_ADAPTER_FILES.contains(&source_file);
    let mut violations = Vec::new();
    let mut visitor = BayesWorkerAuthorityVisitor {
        source_file,
        layer,
        allow_authority,
        worker_boundary: source_file == WORKER_FILE,
        scopes: vec![root_scope],
        current_impl_owner: None,
        worker_bindings: BTreeSet::new(),
        violations: &mut violations,
    };
    visitor.visit_file(&syntax);
    violations.sort();
    Ok(violations)
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum WorkerSymbol {
    Module,
    Type(String),
}

#[derive(Clone, Debug)]
struct WorkerScope {
    module_path: Vec<String>,
    symbols: BTreeMap<String, WorkerSymbol>,
}

#[derive(Clone, Debug)]
struct UseBinding {
    path: Vec<String>,
    local: Option<String>,
    glob: bool,
}

fn source_module_path(source_file: &str) -> Vec<String> {
    let normalized = source_file.replace('\\', "/");
    let relative = normalized
        .strip_prefix("src-tauri/src/")
        .unwrap_or(normalized.as_str());
    let mut segments = relative.split('/').map(str::to_owned).collect::<Vec<_>>();
    let Some(file) = segments.pop() else {
        return Vec::new();
    };
    if !matches!(file.as_str(), "lib.rs" | "main.rs" | "mod.rs") {
        segments.push(file.trim_end_matches(".rs").to_owned());
    }
    segments
}

fn build_worker_scope(
    module_path: Vec<String>,
    items: &[Item],
    parents: &[WorkerScope],
) -> WorkerScope {
    let mut scope = WorkerScope {
        module_path,
        symbols: BTreeMap::new(),
    };
    if is_worker_module_path(&scope.module_path) {
        for owner in WORKER_SURFACE_TYPES {
            scope
                .symbols
                .insert((*owner).to_owned(), WorkerSymbol::Type((*owner).to_owned()));
        }
    }

    let mut use_bindings = Vec::new();
    for item in items {
        if let Item::Use(item_use) = item {
            flatten_use_tree(&item_use.tree, &mut Vec::new(), &mut use_bindings);
        }
    }

    loop {
        let mut scopes = parents.to_vec();
        scopes.push(scope.clone());
        let mut changed = false;

        for binding in &use_bindings {
            let Some(symbol) = resolve_worker_symbol(&binding.path, &scopes) else {
                continue;
            };
            if binding.glob {
                if symbol == WorkerSymbol::Module {
                    for owner in WORKER_SURFACE_TYPES {
                        changed |= insert_worker_symbol(
                            &mut scope.symbols,
                            (*owner).to_owned(),
                            WorkerSymbol::Type((*owner).to_owned()),
                        );
                    }
                }
            } else if let Some(local) = &binding.local {
                changed |= insert_worker_symbol(&mut scope.symbols, local.clone(), symbol);
            }
        }

        let mut scopes = parents.to_vec();
        scopes.push(scope.clone());
        for item in items {
            let Item::Type(item_type) = item else {
                continue;
            };
            if let Some(owner) = canonical_worker_owner_from_type(&item_type.ty, &scopes) {
                changed |= insert_worker_symbol(
                    &mut scope.symbols,
                    item_type.ident.to_string(),
                    WorkerSymbol::Type(owner),
                );
            }
        }

        if !changed {
            break;
        }
    }
    scope
}

fn insert_worker_symbol(
    symbols: &mut BTreeMap<String, WorkerSymbol>,
    local: String,
    symbol: WorkerSymbol,
) -> bool {
    if symbols.get(&local) == Some(&symbol) {
        false
    } else {
        symbols.insert(local, symbol);
        true
    }
}

fn flatten_use_tree(tree: &UseTree, prefix: &mut Vec<String>, bindings: &mut Vec<UseBinding>) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            flatten_use_tree(&path.tree, prefix, bindings);
            prefix.pop();
        }
        UseTree::Name(name) if name.ident == "self" => {
            if let Some(local) = prefix.last().cloned() {
                bindings.push(UseBinding {
                    path: prefix.clone(),
                    local: Some(local),
                    glob: false,
                });
            }
        }
        UseTree::Name(name) => {
            let local = name.ident.to_string();
            let mut path = prefix.clone();
            path.push(local.clone());
            bindings.push(UseBinding {
                path,
                local: Some(local),
                glob: false,
            });
        }
        UseTree::Rename(rename) if rename.ident == "self" => {
            bindings.push(UseBinding {
                path: prefix.clone(),
                local: Some(rename.rename.to_string()),
                glob: false,
            });
        }
        UseTree::Rename(rename) => {
            let mut path = prefix.clone();
            path.push(rename.ident.to_string());
            bindings.push(UseBinding {
                path,
                local: Some(rename.rename.to_string()),
                glob: false,
            });
        }
        UseTree::Group(group) => {
            for item in &group.items {
                flatten_use_tree(item, prefix, bindings);
            }
        }
        UseTree::Glob(_) => bindings.push(UseBinding {
            path: prefix.clone(),
            local: None,
            glob: true,
        }),
    }
}

fn resolve_worker_symbol(path: &[String], scopes: &[WorkerScope]) -> Option<WorkerSymbol> {
    let current = scopes.len().checked_sub(1)?;
    let mut actual_base = scopes[current].module_path.clone();
    let (alias_scope, offset, absolute) = match path.first()?.as_str() {
        "crate" => {
            actual_base.clear();
            (None, 1, true)
        }
        "self" => (Some(current), 1, false),
        "super" => {
            let mut offset = 0;
            while path.get(offset).is_some_and(|segment| segment == "super") {
                offset += 1;
            }
            if offset > actual_base.len() {
                return None;
            }
            actual_base.truncate(actual_base.len() - offset);
            (current.checked_sub(offset), offset, false)
        }
        _ => (Some(current), 0, false),
    };
    let remainder = &path[offset..];
    if remainder.is_empty() {
        return None;
    }

    if !absolute && let Some(scope_index) = alias_scope {
        if let Some(symbol) = scopes[scope_index].symbols.get(&remainder[0]) {
            return extend_worker_symbol(symbol, &remainder[1..]);
        }
    }

    let mut relative = actual_base;
    relative.extend(remainder.iter().cloned());
    if let Some(symbol) = canonical_worker_symbol(&relative) {
        return Some(symbol);
    }
    (!absolute)
        .then(|| canonical_worker_symbol(remainder))
        .flatten()
}

fn extend_worker_symbol(symbol: &WorkerSymbol, suffix: &[String]) -> Option<WorkerSymbol> {
    match (symbol, suffix) {
        (WorkerSymbol::Module, []) => Some(WorkerSymbol::Module),
        (WorkerSymbol::Module, [owner]) if WORKER_SURFACE_TYPES.contains(&owner.as_str()) => {
            Some(WorkerSymbol::Type(owner.clone()))
        }
        (WorkerSymbol::Type(owner), []) => Some(WorkerSymbol::Type(owner.clone())),
        (WorkerSymbol::Module | WorkerSymbol::Type(_), _) => None,
    }
}

fn canonical_worker_symbol(path: &[String]) -> Option<WorkerSymbol> {
    if is_worker_module_path(path) {
        return Some(WorkerSymbol::Module);
    }
    let (owner, prefix) = path.split_last()?;
    if is_worker_module_path(prefix) && WORKER_SURFACE_TYPES.contains(&owner.as_str()) {
        Some(WorkerSymbol::Type(owner.clone()))
    } else {
        None
    }
}

fn is_worker_module_path(segments: &[String]) -> bool {
    let expected = ["sci", "api", "bayes", "worker"];
    segments.iter().map(String::as_str).eq(expected)
}

fn canonical_worker_owner_from_type(ty: &Type, scopes: &[WorkerScope]) -> Option<String> {
    match ty {
        Type::Path(path) => match resolve_worker_symbol(
            &path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>(),
            scopes,
        )? {
            WorkerSymbol::Type(owner) => Some(owner),
            WorkerSymbol::Module => None,
        },
        Type::Reference(reference) => canonical_worker_owner_from_type(&reference.elem, scopes),
        Type::Group(group) => canonical_worker_owner_from_type(&group.elem, scopes),
        Type::Paren(paren) => canonical_worker_owner_from_type(&paren.elem, scopes),
        _ => None,
    }
}

struct BayesWorkerAuthorityVisitor<'a> {
    source_file: &'a str,
    layer: RustLayer,
    allow_authority: bool,
    worker_boundary: bool,
    scopes: Vec<WorkerScope>,
    current_impl_owner: Option<String>,
    worker_bindings: BTreeSet<String>,
    violations: &'a mut Vec<BayesWorkerAuthorityViolation>,
}

impl BayesWorkerAuthorityVisitor<'_> {
    fn record(&mut self, kind: &'static str, target: impl Into<String>) {
        self.violations.push(BayesWorkerAuthorityViolation {
            source_file: self.source_file.to_owned(),
            kind,
            target: target.into(),
        });
    }

    fn layer_forbids_field_access(&self) -> bool {
        matches!(
            self.layer,
            RustLayer::Application
                | RustLayer::Commands
                | RustLayer::Project
                | RustLayer::Graph
                | RustLayer::Execution
        )
    }

    fn inspect_worker_function(&mut self, path: &syn::Path, call: bool) {
        let Some((owner, method)) = self.canonical_owner_and_method(path) else {
            return;
        };
        let target = format!("{owner}::{method}");
        if authority_function(&owner, &method).is_some() {
            if !self.allow_authority {
                self.record(
                    if call {
                        "authority-call"
                    } else {
                        "authority-reference"
                    },
                    target,
                );
            }
        } else if !allowed_worker_function(&owner, &method) {
            self.record("forbidden-associated-function", target);
        }
    }

    fn canonical_owner_and_method(&self, path: &syn::Path) -> Option<(String, String)> {
        let mut segments = path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>();
        let method = segments.pop()?;
        let owner = if matches!(segments.as_slice(), [owner] if owner == "Self") {
            self.current_impl_owner.clone()?
        } else {
            match resolve_worker_symbol(&segments, &self.scopes)? {
                WorkerSymbol::Type(owner) => owner,
                WorkerSymbol::Module => return None,
            }
        };
        Some((owner, method))
    }

    fn inspect_impl_function(&mut self, function: &syn::ImplItemFn) {
        if function.sig.receiver().is_some() {
            return;
        }
        let Some(owner) = self.current_impl_owner.clone() else {
            return;
        };
        let method = function.sig.ident.to_string();
        let target = format!("{owner}::{method}");
        let Some(kind) = disallowed_associated_function_kind(
            &owner,
            &method,
            &function.vis,
            self.worker_boundary,
        ) else {
            return;
        };
        self.record(kind, target);
    }

    fn current_module_path(&self) -> Vec<String> {
        self.scopes
            .last()
            .map(|scope| scope.module_path.clone())
            .unwrap_or_default()
    }

    fn push_inline_scope(&mut self, module: &syn::ItemMod) -> bool {
        let Some((_, items)) = &module.content else {
            return false;
        };
        let mut module_path = self.current_module_path();
        module_path.push(module.ident.to_string());
        let scope = build_worker_scope(module_path, items, &self.scopes);
        self.scopes.push(scope);
        true
    }

    fn pop_inline_scope(&mut self, pushed: bool) {
        if pushed {
            self.scopes.pop();
        }
    }

    fn record_worker_binding(&mut self, pattern: &syn::Pat, ty: &Type) {
        let syn::Pat::Ident(binding) = pattern else {
            return;
        };
        if canonical_worker_owner_from_type(ty, &self.scopes).is_some() {
            self.worker_bindings.insert(binding.ident.to_string());
        }
    }
}

impl<'ast> Visit<'ast> for BayesWorkerAuthorityVisitor<'_> {
    fn visit_item(&mut self, item: &'ast Item) {
        if is_test_only(item_attributes(item)) {
            return;
        }
        visit::visit_item(self, item);
    }

    fn visit_impl_item(&mut self, item: &'ast ImplItem) {
        if is_test_only(impl_item_attributes(item)) {
            return;
        }
        visit::visit_impl_item(self, item);
    }

    fn visit_trait_item(&mut self, item: &'ast TraitItem) {
        if is_test_only(trait_item_attributes(item)) {
            return;
        }
        visit::visit_trait_item(self, item);
    }

    fn visit_expr(&mut self, expression: &'ast Expr) {
        if is_test_only(expr_attributes(expression)) {
            return;
        }
        visit::visit_expr(self, expression);
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        if is_test_only(&item.attrs) {
            return;
        }
        if use_tree_has_worker_glob(&item.tree, &self.scopes) {
            self.record("broad-import", "worker::*");
        }
        visit::visit_item_use(self, item);
    }

    fn visit_item_type(&mut self, item: &'ast syn::ItemType) {
        visit::visit_item_type(self, item);
    }

    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        let pushed = self.push_inline_scope(item);
        visit::visit_item_mod(self, item);
        self.pop_inline_scope(pushed);
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        let previous = self.current_impl_owner.clone();
        self.current_impl_owner = canonical_worker_owner_from_type(&item.self_ty, &self.scopes);
        if self.current_impl_owner.is_some() {
            for impl_item in &item.items {
                if let ImplItem::Fn(function) = impl_item {
                    self.inspect_impl_function(function);
                }
            }
        }
        visit::visit_item_impl(self, item);
        self.current_impl_owner = previous;
    }

    fn visit_item_fn(&mut self, function: &'ast syn::ItemFn) {
        let previous = self.worker_bindings.clone();
        for argument in &function.sig.inputs {
            if let syn::FnArg::Typed(argument) = argument {
                self.record_worker_binding(&argument.pat, &argument.ty);
            }
        }
        visit::visit_item_fn(self, function);
        self.worker_bindings = previous;
    }

    fn visit_impl_item_fn(&mut self, function: &'ast syn::ImplItemFn) {
        let previous = self.worker_bindings.clone();
        for argument in &function.sig.inputs {
            if let syn::FnArg::Typed(argument) = argument {
                self.record_worker_binding(&argument.pat, &argument.ty);
            }
        }
        visit::visit_impl_item_fn(self, function);
        self.worker_bindings = previous;
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        if let syn::Pat::Type(typed) = &local.pat {
            self.record_worker_binding(&typed.pat, &typed.ty);
        }
        visit::visit_local(self, local);
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let Expr::Path(path) = call.func.as_ref() {
            self.inspect_worker_function(&path.path, true);
        }
        visit::visit_expr_call(self, call);
    }

    fn visit_expr_path(&mut self, path: &'ast syn::ExprPath) {
        self.inspect_worker_function(&path.path, false);
        visit::visit_expr_path(self, path);
    }

    fn visit_expr_struct(&mut self, construction: &'ast syn::ExprStruct) {
        if !self.allow_authority {
            let owner = construction.path.segments.last().and_then(|segment| {
                let written = segment.ident.to_string();
                if written == "Self" {
                    self.current_impl_owner.clone()
                } else {
                    match resolve_worker_symbol(
                        &construction
                            .path
                            .segments
                            .iter()
                            .map(|segment| segment.ident.to_string())
                            .collect::<Vec<_>>(),
                        &self.scopes,
                    ) {
                        Some(WorkerSymbol::Type(owner)) => Some(owner),
                        Some(WorkerSymbol::Module) | None => None,
                    }
                }
            });
            if let Some(owner) = owner
                && SEALED_AUTHORITY_TYPES.contains(&owner.as_str())
            {
                self.record("construction", owner);
            }
        }
        visit::visit_expr_struct(self, construction);
    }

    fn visit_expr_field(&mut self, field: &'ast syn::ExprField) {
        if !self.allow_authority
            && self.layer_forbids_field_access()
            && let Expr::Path(receiver) = field.base.as_ref()
            && let Some(binding) = receiver.path.segments.last()
            && self.worker_bindings.contains(&binding.ident.to_string())
            && let syn::Member::Named(member) = &field.member
            && WORKER_PRIVATE_FIELDS.contains(&member.to_string().as_str())
        {
            self.record("field-access", member.to_string());
        }
        visit::visit_expr_field(self, field);
    }
}

fn use_tree_has_worker_glob(tree: &UseTree, scopes: &[WorkerScope]) -> bool {
    let mut bindings = Vec::new();
    flatten_use_tree(tree, &mut Vec::new(), &mut bindings);
    bindings.iter().any(|binding| {
        binding.glob && resolve_worker_symbol(&binding.path, scopes) == Some(WorkerSymbol::Module)
    })
}

fn authority_function(owner: &str, method: &str) -> Option<WorkerFunction> {
    AUTHORITY_FUNCTIONS
        .iter()
        .copied()
        .find(|function| function.owner == owner && function.method == method)
}

fn allowed_worker_function(owner: &str, method: &str) -> bool {
    ALLOWED_WORKER_FUNCTIONS
        .iter()
        .any(|function| function.owner == owner && function.method == method)
}

fn disallowed_associated_function_kind(
    owner: &str,
    method: &str,
    visibility: &Visibility,
    worker_boundary: bool,
) -> Option<&'static str> {
    match visibility {
        Visibility::Inherited => None,
        Visibility::Public(_) => {
            let allowed = worker_boundary
                && PUBLIC_ASSOCIATED_FUNCTIONS
                    .iter()
                    .any(|function| function.owner == owner && function.method == method);
            (!allowed).then_some("public-associated-function")
        }
        Visibility::Restricted(restricted) => {
            let allowed = worker_boundary
                && restricted.in_token.is_none()
                && restricted.path.is_ident("crate")
                && authority_function(owner, method).is_some();
            (!allowed).then_some("restricted-associated-function")
        }
    }
}

fn bayes_worker_surface_violations(
    source: &str,
) -> Result<Vec<BayesWorkerAuthorityViolation>, syn::Error> {
    let syntax = syn::parse_file(source)?;
    let mut violations = Vec::new();
    let mut required_functions = AUTHORITY_FUNCTIONS.iter().copied().collect::<BTreeSet<_>>();

    for item in &syntax.items {
        match item {
            Item::Struct(item_struct)
                if PRIVATE_FIELD_TYPES.contains(&item_struct.ident.to_string().as_str()) =>
            {
                if item_struct
                    .fields
                    .iter()
                    .any(|field| !matches!(field.vis, Visibility::Inherited))
                {
                    violations.push(surface_violation(
                        "public-field",
                        item_struct.ident.to_string(),
                    ));
                }
                if has_forgeable_derive(&item_struct.attrs) {
                    violations.push(surface_violation(
                        "forgeable-derive",
                        item_struct.ident.to_string(),
                    ));
                }
            }
            Item::Impl(item_impl) => {
                let Some(owner) = impl_owner(&item_impl.self_ty) else {
                    continue;
                };
                for impl_item in &item_impl.items {
                    let ImplItem::Fn(function) = impl_item else {
                        continue;
                    };
                    let name = function.sig.ident.to_string();
                    if let Some(authority) = authority_function(&owner, &name) {
                        required_functions.remove(&authority);
                        if !is_crate_visibility(&function.vis) {
                            violations.push(surface_violation(
                                "authority-visibility",
                                format!("{owner}::{name}"),
                            ));
                        }
                    }
                    if function.sig.receiver().is_none()
                        && WORKER_SURFACE_TYPES.contains(&owner.as_str())
                    {
                        if let Some(kind) =
                            disallowed_associated_function_kind(&owner, &name, &function.vis, true)
                        {
                            violations.push(surface_violation(kind, format!("{owner}::{name}")));
                        }
                    }
                }
            }
            Item::Const(_)
            | Item::Enum(_)
            | Item::ExternCrate(_)
            | Item::Fn(_)
            | Item::ForeignMod(_)
            | Item::Macro(_)
            | Item::Mod(_)
            | Item::Static(_)
            | Item::Trait(_)
            | Item::TraitAlias(_)
            | Item::Type(_)
            | Item::Union(_)
            | Item::Use(_)
            | Item::Verbatim(_) => {}
            _ => {}
        }
    }
    for missing in required_functions {
        violations.push(surface_violation(
            "missing-authority-method",
            format!("{}::{}", missing.owner, missing.method),
        ));
    }
    violations.sort();
    Ok(violations)
}

fn surface_violation(
    kind: &'static str,
    target: impl Into<String>,
) -> BayesWorkerAuthorityViolation {
    BayesWorkerAuthorityViolation {
        source_file: WORKER_FILE.to_owned(),
        kind,
        target: target.into(),
    }
}

fn impl_owner(ty: &Type) -> Option<String> {
    let Type::Path(path) = ty else {
        return None;
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

fn is_crate_visibility(visibility: &Visibility) -> bool {
    matches!(
        visibility,
        Visibility::Restricted(restricted)
            if restricted.in_token.is_none() && restricted.path.is_ident("crate")
    )
}

fn has_forgeable_derive(attributes: &[syn::Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        if !attribute.path().is_ident("derive") {
            return false;
        }
        attribute
            .parse_args_with(Punctuated::<syn::Path, Token![,]>::parse_terminated)
            .is_ok_and(|derives| {
                derives.iter().any(|derive| {
                    derive.segments.last().is_some_and(|segment| {
                        matches!(
                            segment.ident.to_string().as_str(),
                            "Default" | "Deserialize"
                        )
                    })
                })
            })
    })
}

fn item_attributes(item: &Item) -> &[syn::Attribute] {
    match item {
        Item::Const(item) => &item.attrs,
        Item::Enum(item) => &item.attrs,
        Item::ExternCrate(item) => &item.attrs,
        Item::Fn(item) => &item.attrs,
        Item::ForeignMod(item) => &item.attrs,
        Item::Impl(item) => &item.attrs,
        Item::Macro(item) => &item.attrs,
        Item::Mod(item) => &item.attrs,
        Item::Static(item) => &item.attrs,
        Item::Struct(item) => &item.attrs,
        Item::Trait(item) => &item.attrs,
        Item::TraitAlias(item) => &item.attrs,
        Item::Type(item) => &item.attrs,
        Item::Union(item) => &item.attrs,
        Item::Use(item) => &item.attrs,
        Item::Verbatim(_) => &[],
        _ => &[],
    }
}

fn impl_item_attributes(item: &ImplItem) -> &[syn::Attribute] {
    match item {
        ImplItem::Const(item) => &item.attrs,
        ImplItem::Fn(item) => &item.attrs,
        ImplItem::Type(item) => &item.attrs,
        ImplItem::Macro(item) => &item.attrs,
        ImplItem::Verbatim(_) => &[],
        _ => &[],
    }
}

fn trait_item_attributes(item: &TraitItem) -> &[syn::Attribute] {
    match item {
        TraitItem::Const(item) => &item.attrs,
        TraitItem::Fn(item) => &item.attrs,
        TraitItem::Type(item) => &item.attrs,
        TraitItem::Macro(item) => &item.attrs,
        TraitItem::Verbatim(_) => &[],
        _ => &[],
    }
}

fn expr_attributes(expression: &Expr) -> &[syn::Attribute] {
    match expression {
        Expr::Array(expression) => &expression.attrs,
        Expr::Assign(expression) => &expression.attrs,
        Expr::Async(expression) => &expression.attrs,
        Expr::Await(expression) => &expression.attrs,
        Expr::Binary(expression) => &expression.attrs,
        Expr::Block(expression) => &expression.attrs,
        Expr::Break(expression) => &expression.attrs,
        Expr::Call(expression) => &expression.attrs,
        Expr::Cast(expression) => &expression.attrs,
        Expr::Closure(expression) => &expression.attrs,
        Expr::Const(expression) => &expression.attrs,
        Expr::Continue(expression) => &expression.attrs,
        Expr::Field(expression) => &expression.attrs,
        Expr::ForLoop(expression) => &expression.attrs,
        Expr::Group(expression) => &expression.attrs,
        Expr::If(expression) => &expression.attrs,
        Expr::Index(expression) => &expression.attrs,
        Expr::Infer(expression) => &expression.attrs,
        Expr::Let(expression) => &expression.attrs,
        Expr::Lit(expression) => &expression.attrs,
        Expr::Loop(expression) => &expression.attrs,
        Expr::Macro(expression) => &expression.attrs,
        Expr::Match(expression) => &expression.attrs,
        Expr::MethodCall(expression) => &expression.attrs,
        Expr::Paren(expression) => &expression.attrs,
        Expr::Path(expression) => &expression.attrs,
        Expr::Range(expression) => &expression.attrs,
        Expr::RawAddr(expression) => &expression.attrs,
        Expr::Reference(expression) => &expression.attrs,
        Expr::Repeat(expression) => &expression.attrs,
        Expr::Return(expression) => &expression.attrs,
        Expr::Struct(expression) => &expression.attrs,
        Expr::Try(expression) => &expression.attrs,
        Expr::TryBlock(expression) => &expression.attrs,
        Expr::Tuple(expression) => &expression.attrs,
        Expr::Unary(expression) => &expression.attrs,
        Expr::Unsafe(expression) => &expression.attrs,
        Expr::While(expression) => &expression.attrs,
        Expr::Yield(expression) => &expression.attrs,
        _ => &[],
    }
}

fn bayes_worker_result_neutrality_violations(source: &str) -> Result<Vec<String>, syn::Error> {
    const FORBIDDEN_TYPES: &[&str] = &[
        "InferenceResult",
        "ResultArtifactManifest",
        "ResultArtifact",
        "JuliaWorkerTaskDirectory",
        "Path",
        "PathBuf",
    ];

    let syntax = syn::parse_file(source)?;
    let mut findings = BTreeSet::new();
    for item in &syntax.items {
        match item {
            Item::Use(item_use) => {
                collect_forbidden_use_names(&item_use.tree, FORBIDDEN_TYPES, &mut findings);
            }
            Item::Struct(item_struct) if item_struct.ident == "BayesTaskResult" => {
                require_exact_fields(
                    item_struct,
                    &["artifacts", "inference"],
                    "BayesTaskResult-fields",
                    &mut findings,
                );
                collect_forbidden_field_types(item_struct, FORBIDDEN_TYPES, &mut findings);
            }
            Item::Struct(item_struct) if item_struct.ident == "BayesInferenceSnapshot" => {
                require_exact_fields(
                    item_struct,
                    &["diagnostics", "summaries", "task"],
                    "BayesInferenceSnapshot-fields",
                    &mut findings,
                );
                collect_forbidden_field_types(item_struct, FORBIDDEN_TYPES, &mut findings);
            }
            _ => {}
        }
    }
    Ok(findings.into_iter().collect())
}

fn collect_forbidden_use_names(
    tree: &UseTree,
    forbidden: &[&str],
    findings: &mut BTreeSet<String>,
) {
    match tree {
        UseTree::Path(path) => collect_forbidden_use_names(&path.tree, forbidden, findings),
        UseTree::Name(name) => {
            let name = name.ident.to_string();
            if forbidden.contains(&name.as_str()) {
                findings.insert(name);
            }
        }
        UseTree::Rename(rename) => {
            let name = rename.ident.to_string();
            if forbidden.contains(&name.as_str()) {
                findings.insert(name);
            }
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_forbidden_use_names(item, forbidden, findings);
            }
        }
        UseTree::Glob(_) => {}
    }
}

fn require_exact_fields(
    item: &syn::ItemStruct,
    expected: &[&str],
    finding: &str,
    findings: &mut BTreeSet<String>,
) {
    let actual = item
        .fields
        .iter()
        .filter_map(|field| field.ident.as_ref().map(ToString::to_string))
        .collect::<BTreeSet<_>>();
    let expected = expected
        .iter()
        .map(|field| (*field).to_owned())
        .collect::<BTreeSet<_>>();
    if actual != expected {
        findings.insert(finding.to_owned());
    }
}

fn collect_forbidden_field_types(
    item: &syn::ItemStruct,
    forbidden: &[&str],
    findings: &mut BTreeSet<String>,
) {
    let mut visitor = ForbiddenResultTypeVisitor {
        forbidden,
        findings,
    };
    for field in &item.fields {
        visitor.visit_type(&field.ty);
    }
}

struct ForbiddenResultTypeVisitor<'a> {
    forbidden: &'a [&'a str],
    findings: &'a mut BTreeSet<String>,
}

impl<'ast> Visit<'ast> for ForbiddenResultTypeVisitor<'_> {
    fn visit_type_path(&mut self, path: &'ast syn::TypePath) {
        for segment in &path.path.segments {
            let name = segment.ident.to_string();
            if self.forbidden.contains(&name.as_str()) {
                self.findings.insert(name);
            }
        }
        visit::visit_type_path(self, path);
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct JuliaBayesAdapterViolation {
    source_file: String,
    kind: &'static str,
    target: String,
}

fn julia_bayes_adapter_source_violations(
    source_file: &str,
    source: &str,
) -> Result<Vec<JuliaBayesAdapterViolation>, syn::Error> {
    let syntax = syn::parse_file(source)?;
    let mut violations = Vec::new();
    let adapter_source = JULIA_WORKER_ADAPTER_FILES.contains(&source_file);
    let mut visitor = JuliaBayesAdapterVisitor {
        source_file,
        adapter_source,
        violations: &mut violations,
    };
    visitor.visit_file(&syntax);
    violations.sort();
    Ok(violations)
}

struct JuliaBayesAdapterVisitor<'a> {
    source_file: &'a str,
    adapter_source: bool,
    violations: &'a mut Vec<JuliaBayesAdapterViolation>,
}

impl JuliaBayesAdapterVisitor<'_> {
    fn record(&mut self, kind: &'static str, target: impl Into<String>) {
        self.violations.push(JuliaBayesAdapterViolation {
            source_file: self.source_file.to_owned(),
            kind,
            target: target.into(),
        });
    }

    fn inspect_path(&mut self, path: &syn::Path) {
        const OLD_ROUTE_TYPES: &[&str] = &[
            "BayesBackend",
            "BayesBackendRequest",
            "InferenceResult",
            "ResultArtifact",
            "ResultArtifactManifest",
        ];

        for segment in &path.segments {
            let name = segment.ident.to_string();
            if self.adapter_source && OLD_ROUTE_TYPES.contains(&name.as_str()) {
                self.record("old-route-origin", name);
            }
        }
        if !self.adapter_source
            && path
                .segments
                .iter()
                .any(|segment| segment.ident == "JuliaBayesWorkerAdapter")
        {
            self.record("production-reference", "JuliaBayesWorkerAdapter");
        }
    }
}

impl<'ast> Visit<'ast> for JuliaBayesAdapterVisitor<'_> {
    fn visit_item(&mut self, item: &'ast Item) {
        if is_test_only(item_attributes(item)) {
            return;
        }
        visit::visit_item(self, item);
    }

    fn visit_impl_item(&mut self, item: &'ast ImplItem) {
        if is_test_only(impl_item_attributes(item)) {
            return;
        }
        visit::visit_impl_item(self, item);
    }

    fn visit_expr(&mut self, expression: &'ast Expr) {
        if is_test_only(expr_attributes(expression)) {
            return;
        }
        visit::visit_expr(self, expression);
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        if is_test_only(&item.attrs) {
            return;
        }
        if self.adapter_source
            && impl_owner(&item.self_ty).as_deref() == Some("JuliaBayesWorkerAdapter")
            && let Some((trait_path, _)) = &item.trait_
            && trait_path
                .segments
                .last()
                .is_none_or(|segment| segment.ident != "BayesWorkerPort")
        {
            self.record(
                "non-port-trait",
                trait_path
                    .segments
                    .last()
                    .map_or_else(String::new, |segment| segment.ident.to_string()),
            );
        }
        visit::visit_item_impl(self, item);
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        self.inspect_path(path);
        visit::visit_path(self, path);
    }
}

#[test]
fn julia_bayes_worker_adapter_is_port_only_and_production_unreachable() {
    let staged_debt = super::debt::staged_backend_adapter_debt();
    assert_eq!(staged_debt.len(), 2);
    assert_eq!(
        staged_debt[0].adapter,
        "yssbi_lib::julia::bayes_worker_adapter::JuliaBayesWorkerAdapter"
    );
    assert_eq!(staged_debt[0].activation_owner, "Execution Task 8");
    assert_eq!(
        staged_debt[0].owning_migration_spec,
        "docs/architecture/RUST_BACKEND_ADAPTER_BOUNDARIES.md"
    );

    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let workspace = super::cargo_targets::discover_rust_workspace_model(&manifest)
        .expect("the real Cargo workspace must be discoverable");
    let modules = collect_production_modules(&workspace.repository_root, &workspace.roots)
        .expect("the production module graph must be discoverable");
    let source_files = modules
        .iter()
        .map(|module| module.repository_relative_source_file.as_str())
        .collect::<BTreeSet<_>>();
    let missing = JULIA_WORKER_ADAPTER_FILES
        .iter()
        .filter(|source_file| !source_files.contains(**source_file))
        .copied()
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "the final Julia Bayes adapter files must be production modules: {missing:#?}"
    );

    let mut actual = Vec::new();
    for source_file in source_files {
        let path = workspace.repository_root.join(source_file);
        let source = std::fs::read_to_string(&path).expect("production source must be readable");
        actual.extend(
            julia_bayes_adapter_source_violations(source_file, &source)
                .expect("production source must parse"),
        );
    }
    actual.sort();
    assert!(
        actual.is_empty(),
        "the final Julia Bayes adapter escaped its staged port-only boundary: {actual:#?}"
    );

    let fixture = julia_bayes_adapter_source_violations(
        JULIA_WORKER_ADAPTER_FILES[0],
        r#"
use crate::sci::api::bayes::{BayesBackend, BayesBackendRequest, InferenceResult};

pub struct JuliaBayesWorkerAdapter;
impl BayesBackend for JuliaBayesWorkerAdapter {}
impl BayesWorkerPort for JuliaBayesWorkerAdapter {}
"#,
    )
    .expect("adapter authority fixture must parse");
    assert!(
        fixture.iter().any(|finding| {
            finding.kind == "non-port-trait" && finding.target == "BayesBackend"
        })
    );
    assert!(
        fixture
            .iter()
            .any(|finding| finding.kind == "old-route-origin")
    );

    let production_constructor = julia_bayes_adapter_source_violations(
        "src-tauri/src/lib.rs",
        "fn compose() { let _ = JuliaBayesWorkerAdapter::new(root, worker); }",
    )
    .expect("production constructor fixture must parse");
    assert!(production_constructor.iter().any(|finding| {
        finding.kind == "production-reference" && finding.target == "JuliaBayesWorkerAdapter"
    }));
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ScientificAdapterViolation {
    source_file: String,
    kind: &'static str,
    target: String,
}

fn scientific_adapter_source_violations(
    source_file: &str,
    source: &str,
) -> Result<Vec<ScientificAdapterViolation>, syn::Error> {
    let syntax = syn::parse_file(source)?;
    let mut violations = Vec::new();
    let mut visitor = ScientificAdapterVisitor {
        source_file,
        adapter_source: source_file == SCIENTIFIC_ADAPTER_FILE,
        port_source: source_file == SCIENTIFIC_PORT_FILE,
        violations: &mut violations,
    };
    visitor.visit_file(&syntax);
    violations.sort();
    Ok(violations)
}

struct ScientificAdapterVisitor<'a> {
    source_file: &'a str,
    adapter_source: bool,
    port_source: bool,
    violations: &'a mut Vec<ScientificAdapterViolation>,
}

impl ScientificAdapterVisitor<'_> {
    fn record(&mut self, kind: &'static str, target: impl Into<String>) {
        self.violations.push(ScientificAdapterViolation {
            source_file: self.source_file.to_owned(),
            kind,
            target: target.into(),
        });
    }

    fn inspect_segments(&mut self, segments: &[String]) {
        if self.adapter_source {
            for forbidden in ["engine", "models", "kde", "backends"] {
                if segments.windows(2).any(|pair| pair == ["sci", forbidden]) {
                    self.record("forbidden-sci-owner", format!("sci::{forbidden}"));
                }
            }
        }
        if self.port_source && segments.iter().any(|segment| segment == "sci") {
            self.record("execution-imports-sci", segments.join("::"));
        }
        if !self.adapter_source
            && segments
                .iter()
                .any(|segment| segment == "SciApiScientificBackend")
        {
            self.record("production-reference", "SciApiScientificBackend");
        }
    }
}

impl<'ast> Visit<'ast> for ScientificAdapterVisitor<'_> {
    fn visit_item(&mut self, item: &'ast Item) {
        if is_test_only(item_attributes(item)) {
            return;
        }
        visit::visit_item(self, item);
    }

    fn visit_impl_item(&mut self, item: &'ast ImplItem) {
        if is_test_only(impl_item_attributes(item)) {
            return;
        }
        visit::visit_impl_item(self, item);
    }

    fn visit_expr(&mut self, expression: &'ast Expr) {
        if is_test_only(expr_attributes(expression)) {
            return;
        }
        visit::visit_expr(self, expression);
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        if is_test_only(&item.attrs) {
            return;
        }
        if self.adapter_source
            && impl_owner(&item.self_ty).as_deref() == Some("SciApiScientificBackend")
            && let Some((trait_path, _)) = &item.trait_
            && trait_path
                .segments
                .last()
                .is_none_or(|segment| segment.ident != "ScientificBackend")
        {
            self.record(
                "non-port-trait",
                trait_path
                    .segments
                    .last()
                    .map_or_else(String::new, |segment| segment.ident.to_string()),
            );
        }
        visit::visit_item_impl(self, item);
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        if is_test_only(&item.attrs) {
            return;
        }
        let mut bindings = Vec::new();
        flatten_use_tree(&item.tree, &mut Vec::new(), &mut bindings);
        for binding in bindings {
            self.inspect_segments(&binding.path);
        }
        visit::visit_item_use(self, item);
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        let segments = path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>();
        self.inspect_segments(&segments);
        visit::visit_path(self, path);
    }
}

#[test]
fn scientific_backend_adapter_is_exact_port_only_and_production_unreachable() {
    let staged_debt = super::debt::staged_backend_adapter_debt();
    let debt = staged_debt
        .iter()
        .find(|debt| debt.adapter.ends_with("::SciApiScientificBackend"))
        .expect("the staged scientific adapter must retain activation debt");
    assert_eq!(debt.activation_owner, "Execution Task 8");
    assert_eq!(
        debt.owning_migration_spec,
        "docs/architecture/RUST_BACKEND_ADAPTER_BOUNDARIES.md"
    );

    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let workspace = super::cargo_targets::discover_rust_workspace_model(&manifest)
        .expect("the real Cargo workspace must be discoverable");
    let modules = collect_production_modules(&workspace.repository_root, &workspace.roots)
        .expect("the production module graph must be discoverable");
    let source_files = modules
        .iter()
        .map(|module| module.repository_relative_source_file.as_str())
        .collect::<BTreeSet<_>>();
    let missing = SCIENTIFIC_BOUNDARY_FILES
        .iter()
        .filter(|source_file| !source_files.contains(**source_file))
        .copied()
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "the final scientific boundary files must be production modules: {missing:#?}"
    );

    let mut actual = Vec::new();
    for source_file in source_files {
        let path = workspace.repository_root.join(source_file);
        let source = std::fs::read_to_string(&path).expect("production source must be readable");
        actual.extend(
            scientific_adapter_source_violations(source_file, &source)
                .expect("production source must parse"),
        );
    }
    actual.sort();
    assert!(
        actual.is_empty(),
        "the final scientific adapter escaped its exact staged boundary: {actual:#?}"
    );

    let adapter_fixture = scientific_adapter_source_violations(
        SCIENTIFIC_ADAPTER_FILE,
        r#"
use crate::sci::engine::SciContext;
struct SciApiScientificBackend;
impl LegacyScientificBackend for SciApiScientificBackend {}
"#,
    )
    .expect("adapter fixture must parse");
    assert!(
        adapter_fixture
            .iter()
            .any(|finding| finding.kind == "forbidden-sci-owner")
    );
    assert!(
        adapter_fixture
            .iter()
            .any(|finding| finding.kind == "non-port-trait")
    );

    let port_fixture = scientific_adapter_source_violations(
        SCIENTIFIC_PORT_FILE,
        "use crate::sci::api::node_statistics::RegressionKind;",
    )
    .expect("port fixture must parse");
    assert!(
        port_fixture
            .iter()
            .any(|finding| finding.kind == "execution-imports-sci")
    );

    let production_constructor = scientific_adapter_source_violations(
        "src-tauri/src/lib.rs",
        "fn compose() { let _ = SciApiScientificBackend::new(); }",
    )
    .expect("production constructor fixture must parse");
    assert!(production_constructor.iter().any(|finding| {
        finding.kind == "production-reference" && finding.target == "SciApiScientificBackend"
    }));
}

#[test]
fn bayes_artifact_authority_can_only_be_minted_at_worker_boundary() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let workspace = super::cargo_targets::discover_rust_workspace_model(&manifest)
        .expect("the real Cargo workspace must be discoverable");
    let modules = collect_production_modules(&workspace.repository_root, &workspace.roots)
        .expect("the production module graph must be discoverable");
    let classification = classify_rust_sources(&workspace.roots, &modules)
        .expect("every production source must classify exactly once");

    let actual =
        bayes_worker_authority_violations(&workspace.repository_root, &modules, &classification)
            .expect("the Bayes worker authority guard must audit the real source graph");
    assert!(
        actual.is_empty(),
        "Bayes worker authority escaped its exact boundary: {actual:#?}"
    );

    let fixture = bayes_worker_source_violations(
        "src-tauri/src/application/forbidden.rs",
        RustLayer::Application,
        r#"
use crate::sci::api::bayes::worker::*;

struct WrongOwner;

impl WrongOwner {
    fn issue_for_worker() {}
}

fn forge(result: BayesTaskResult) {
    let _ = BayesTaskHandle { task_id, generation };
    let _ = BayesTaskHandle::issue_for_worker(task_id, generation);
    let issue = BayesTaskHandle::issue_for_worker;
    let _ = issue(task_id, generation);
    WrongOwner::issue_for_worker();
    let _ = BayesTaskHandle::from_parts(task_id, generation);
    let _ = BayesTaskHandle::forge(task_id, generation);
    let _ = result.task;
}
"#,
    )
    .expect("the fixture must parse");
    assert!(fixture.iter().any(|finding| finding.kind == "broad-import"));
    assert!(fixture.iter().any(|finding| finding.kind == "construction"));
    assert!(fixture.iter().any(|finding| {
        finding.kind == "authority-call" && finding.target == "BayesTaskHandle::issue_for_worker"
    }));
    assert!(fixture.iter().any(|finding| {
        finding.kind == "authority-reference"
            && finding.target == "BayesTaskHandle::issue_for_worker"
    }));
    assert!(fixture.iter().any(|finding| {
        finding.kind == "forbidden-associated-function"
            && finding.target == "BayesTaskHandle::from_parts"
    }));
    assert!(fixture.iter().any(|finding| {
        finding.kind == "forbidden-associated-function"
            && finding.target == "BayesTaskHandle::forge"
    }));
    assert!(
        !fixture
            .iter()
            .any(|finding| finding.target == "WrongOwner::issue_for_worker")
    );
    assert!(fixture.iter().any(|finding| finding.kind == "field-access"));

    let surface_fixture = bayes_worker_surface_violations(
        r#"
pub struct BayesTaskHandle { task_id: (), generation: () }
impl BayesTaskHandle {
    pub(crate) fn issue_for_worker() {}
    pub fn task_id(&self) {}
    pub fn generation(&self) {}
    pub fn from_parts() {}
    pub fn forge() {}
}
pub struct BayesArtifactHandle { task: (), artifact: () }
impl BayesArtifactHandle { pub(crate) fn mint_for_worker() {} }
pub struct BayesArtifact { handle: (), media_type: (), bytes: () }
impl BayesArtifact { pub(crate) fn from_worker() {} }
pub struct BayesTaskResult { inference: (), artifacts: () }
impl BayesTaskResult { pub(crate) fn validated_worker_result() {} }
"#,
    )
    .expect("the authority surface fixture must parse");
    assert!(surface_fixture.iter().any(|finding| {
        finding.kind == "public-associated-function"
            && finding.target == "BayesTaskHandle::from_parts"
    }));
    assert!(surface_fixture.iter().any(|finding| {
        finding.kind == "public-associated-function" && finding.target == "BayesTaskHandle::forge"
    }));

    let worker_source = std::fs::read_to_string(
        workspace
            .repository_root
            .join("src-tauri/src/sci/api/bayes/worker.rs"),
    )
    .expect("Bayes worker contract source must be readable");
    let surface = bayes_worker_surface_violations(&worker_source)
        .expect("Bayes worker contract source must parse");
    assert!(
        surface.is_empty(),
        "Bayes worker authority/result fields or constructors became public: {surface:#?}"
    );
}

#[test]
fn bayes_import_and_type_alias_authority_is_rejected() {
    let findings = bayes_worker_source_violations(
        "src-tauri/src/application/aliased_worker_authority.rs",
        RustLayer::Application,
        r#"
use crate::sci::api::bayes::worker::BayesTaskHandle as Handle;
type TaskAuthority = Handle;

fn forge(task_id: BayesTaskId, generation: NonZeroU64) {
    let _ = Handle::issue_for_worker(task_id.clone(), generation);
    let issue = TaskAuthority::issue_for_worker;
    let _ = issue(task_id, generation);
}
"#,
    )
    .expect("aliased authority fixture must parse");
    assert!(findings.iter().any(|finding| {
        finding.kind == "authority-call" && finding.target == "BayesTaskHandle::issue_for_worker"
    }));
    assert!(findings.iter().any(|finding| {
        finding.kind == "authority-reference"
            && finding.target == "BayesTaskHandle::issue_for_worker"
    }));
}

#[test]
fn bayes_external_impl_self_authority_is_rejected() {
    let findings = bayes_worker_source_violations(
        "src-tauri/src/application/external_worker_impl.rs",
        RustLayer::Application,
        r#"
use crate::sci::api::bayes::worker::BayesTaskHandle;

impl BayesTaskHandle {
    pub fn forge(task_id: BayesTaskId, generation: NonZeroU64) -> Self {
        Self::issue_for_worker(task_id, generation)
    }
}
"#,
    )
    .expect("external authority impl fixture must parse");
    assert!(findings.iter().any(|finding| {
        finding.kind == "public-associated-function" && finding.target == "BayesTaskHandle::forge"
    }));
    assert!(findings.iter().any(|finding| {
        finding.kind == "authority-call" && finding.target == "BayesTaskHandle::issue_for_worker"
    }));
}

#[test]
fn bayes_module_scoped_aliases_are_canonicalized() {
    let module_alias = bayes_worker_source_violations(
        "src-tauri/src/application/module_alias_authority.rs",
        RustLayer::Application,
        r#"
use crate::sci::api::bayes::worker as w;
use w::BayesTaskHandle as Handle;

fn forge(task_id: BayesTaskId, generation: NonZeroU64) {
    let _ = Handle::issue_for_worker(task_id, generation);
}
"#,
    )
    .expect("module alias fixture must parse");
    assert!(module_alias.iter().any(|finding| {
        finding.kind == "authority-call" && finding.target == "BayesTaskHandle::issue_for_worker"
    }));

    let relative_import = bayes_worker_source_violations(
        "src-tauri/src/sci/api/bayes/forbidden_authority.rs",
        RustLayer::Application,
        r#"
use super::worker::BayesArtifactHandle as RelativeArtifact;

fn forge(task: BayesTaskHandle, artifact: ArtifactId) {
    let _ = RelativeArtifact::mint_for_worker(task, artifact);
}
"#,
    )
    .expect("relative authority fixture must parse");
    assert!(relative_import.iter().any(|finding| {
        finding.kind == "authority-call" && finding.target == "BayesArtifactHandle::mint_for_worker"
    }));

    let nested_forward_alias = bayes_worker_source_violations(
        "src-tauri/src/application/nested_alias_authority.rs",
        RustLayer::Application,
        r#"
use crate::sci::api::bayes::worker as w;

mod nested {
    type A = B;
    type B = super::w::BayesTaskResult;

    fn forge() {
        let build = A::validated_worker_result;
    }
}
"#,
    )
    .expect("nested forward alias fixture must parse");
    assert!(nested_forward_alias.iter().any(|finding| {
        finding.kind == "authority-reference"
            && finding.target == "BayesTaskResult::validated_worker_result"
    }));

    let unrelated = bayes_worker_source_violations(
        "src-tauri/src/application/unrelated_handle.rs",
        RustLayer::Application,
        r#"
use other::Handle;

fn allowed() {
    Handle::issue_for_worker();
}
"#,
    )
    .expect("unrelated handle fixture must parse");
    assert!(
        unrelated.is_empty(),
        "unrelated Handle must not canonicalize"
    );
}

#[test]
fn bayes_group_self_module_alias_is_canonicalized() {
    let findings = bayes_worker_source_violations(
        "src-tauri/src/application/group_self_authority.rs",
        RustLayer::Application,
        r#"
use crate::sci::api::bayes::worker::{self as w};
use w::BayesTaskHandle as Handle;

fn forge(task_id: BayesTaskId, generation: NonZeroU64) {
    let _ = Handle::issue_for_worker(task_id, generation);
}
"#,
    )
    .expect("group self authority fixture must parse");
    assert!(findings.iter().any(|finding| {
        finding.kind == "authority-call" && finding.target == "BayesTaskHandle::issue_for_worker"
    }));
}

#[test]
fn bayes_restricted_associated_functions_are_rejected() {
    let findings = bayes_worker_source_violations(
        "src-tauri/src/application/restricted_worker_impl.rs",
        RustLayer::Application,
        r#"
use crate::sci::api::bayes::worker::BayesTaskHandle;

impl BayesTaskHandle {
    pub(super) fn from_parts() {}
    pub(in crate::application) fn forge() {}
    pub(in crate) fn issue_for_worker() {}
}
"#,
    )
    .expect("restricted authority fixture must parse");
    assert!(findings.iter().any(|finding| {
        finding.kind == "restricted-associated-function"
            && finding.target == "BayesTaskHandle::from_parts"
    }));
    assert!(findings.iter().any(|finding| {
        finding.kind == "restricted-associated-function"
            && finding.target == "BayesTaskHandle::forge"
    }));
    assert!(findings.iter().any(|finding| {
        finding.kind == "restricted-associated-function"
            && finding.target == "BayesTaskHandle::issue_for_worker"
    }));
}

#[test]
fn bayes_model_spec_exposes_only_final_adapter_projection() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let workspace = super::cargo_targets::discover_rust_workspace_model(&manifest)
        .expect("the real Cargo workspace must be discoverable");
    let path = workspace
        .repository_root
        .join("src-tauri/src/sci/api/bayes/model.rs");
    let source = std::fs::read_to_string(&path).expect("Bayes model source must be readable");
    let syntax = syn::parse_file(&source).expect("Bayes model source must parse");

    let model = syntax
        .items
        .iter()
        .find_map(|item| match item {
            Item::Struct(item) if item.ident == "BayesModelSpec" => Some(item),
            _ => None,
        })
        .expect("BayesModelSpec must exist");
    let visible_fields = model
        .fields
        .iter()
        .filter(|field| !matches!(field.vis, Visibility::Inherited))
        .filter_map(|field| field.ident.as_ref().map(ToString::to_string))
        .collect::<BTreeSet<_>>();
    assert!(
        visible_fields.is_empty(),
        "BayesModelSpec fields must stay private: {visible_fields:#?}"
    );

    let public_methods = syntax
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Impl(item) if impl_owner(&item.self_ty).as_deref() == Some("BayesModelSpec") => {
                Some(item)
            }
            _ => None,
        })
        .flat_map(|item| item.items.iter())
        .filter_map(|item| match item {
            ImplItem::Fn(function) if matches!(function.vis, Visibility::Public(_)) => {
                Some(function.sig.ident.to_string())
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        public_methods,
        BTreeSet::from([
            "data_variables".to_owned(),
            "likelihood".to_owned(),
            "parameters".to_owned(),
            "predictor".to_owned(),
            "sampler".to_owned(),
        ]),
        "BayesModelSpec public capability must stay exact"
    );

    let restricted_methods = syntax
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Impl(item) if impl_owner(&item.self_ty).as_deref() == Some("BayesModelSpec") => {
                Some(item)
            }
            _ => None,
        })
        .flat_map(|item| item.items.iter())
        .filter_map(|item| match item {
            ImplItem::Fn(function) => match &function.vis {
                Visibility::Restricted(restricted) => Some((
                    function.sig.ident.to_string(),
                    restricted
                        .path
                        .segments
                        .iter()
                        .map(|segment| segment.ident.to_string())
                        .collect::<Vec<_>>()
                        .join("::"),
                )),
                Visibility::Public(_) | Visibility::Inherited => None,
            },
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        restricted_methods,
        BTreeMap::from([
            ("dataset".to_owned(), "crate".to_owned()),
            ("display_formula".to_owned(), "crate".to_owned()),
            ("from_validated_parts".to_owned(), "super".to_owned()),
            ("parameter_names".to_owned(), "super".to_owned()),
            ("response".to_owned(), "crate".to_owned()),
        ]),
        "BayesModelSpec internal construction and old-route projection must stay exact"
    );
}

#[test]
fn bayes_worker_result_is_neutral_and_path_free() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let workspace = super::cargo_targets::discover_rust_workspace_model(&manifest)
        .expect("the real Cargo workspace must be discoverable");
    let worker_source = std::fs::read_to_string(
        workspace
            .repository_root
            .join("src-tauri/src/sci/api/bayes/worker.rs"),
    )
    .expect("Bayes worker source must be readable");
    let actual = bayes_worker_result_neutrality_violations(&worker_source)
        .expect("Bayes worker source must parse");
    assert!(
        actual.is_empty(),
        "Bayes worker result must not own legacy result/path authority: {actual:#?}"
    );

    let contract_source = std::fs::read_to_string(
        workspace
            .repository_root
            .join("src-tauri/src/sci/api/bayes/contract.rs"),
    )
    .expect("Bayes neutral contract source must be readable");
    let result_source = std::fs::read_to_string(
        workspace
            .repository_root
            .join("src-tauri/src/sci/api/bayes/result.rs"),
    )
    .expect("Bayes legacy result source must be readable");
    let contract = syn::parse_file(&contract_source).expect("Bayes neutral contract must parse");
    let result = syn::parse_file(&result_source).expect("Bayes legacy result must parse");
    for symbol in [
        "ParameterSummary",
        "InferenceDiagnostics",
        "DiagnosticWarning",
        "DiagnosticMetric",
    ] {
        let in_contract = contract.items.iter().any(|item| match item {
            Item::Struct(item) => item.ident == symbol,
            Item::Enum(item) => item.ident == symbol,
            _ => false,
        });
        let in_legacy_result = result.items.iter().any(|item| match item {
            Item::Struct(item) => item.ident == symbol,
            Item::Enum(item) => item.ident == symbol,
            Item::Type(item) => item.ident == symbol,
            _ => false,
        });
        assert!(in_contract, "{symbol} must be neutral-contract owned");
        assert!(
            !in_legacy_result,
            "{symbol} must not be declared or aliased by the old result owner"
        );
    }
    for owner in [
        "InferenceResult",
        "ResultArtifactManifest",
        "ResultArtifact",
    ] {
        let item = result
            .items
            .iter()
            .find_map(|item| match item {
                Item::Struct(item) if item.ident == owner => Some(item),
                _ => None,
            })
            .expect("legacy result owner must exist");
        assert!(
            item.fields
                .iter()
                .all(|field| matches!(field.vis, Visibility::Inherited)),
            "{owner} fields must stay private behind canonical getters"
        );
    }

    let fixture = bayes_worker_result_neutrality_violations(
        r#"
use crate::sci::api::bayes::result::{InferenceResult, ResultArtifactManifest};
use std::path::PathBuf;

pub struct BayesInferenceSnapshot {
    inference: InferenceResult,
    artifact_manifest: ResultArtifactManifest,
    path: PathBuf,
}

pub struct BayesTaskResult {
    inference: InferenceResult,
    artifacts: std::sync::Arc<[BayesArtifactHandle]>,
}
"#,
    )
    .expect("legacy result fixture must parse");
    assert!(fixture.iter().any(|finding| finding == "InferenceResult"));
    assert!(
        fixture
            .iter()
            .any(|finding| finding == "ResultArtifactManifest")
    );
    assert!(fixture.iter().any(|finding| finding == "PathBuf"));
}
