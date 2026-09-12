use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use syn::{Item, Type};

use super::cargo_targets::rust_workspace_model_from_metadata;
use super::dependency_audit::{
    collect_production_dependencies, collect_production_modules,
    resolve_canonical_dependencies_detailed,
};
use super::external_policy::{
    ExternalDependencyDeclarationAllowance, ExternalDependencyPolicy,
    ExternalDependencyUseAllowance, RUST_EXTERNAL_DEPENDENCY_POLICY,
    rust_external_dependency_findings,
};
use super::model::{
    ArchitectureAuditError, CanonicalDependency, CanonicalOrigin, CargoDependencyAuthority,
    CargoDependencyDeclaration, CargoDependencyScope, ExternalDependencyOrigin, ProductionRoot,
    ProductionRootKind, RustDependencyKind, RustDependencyMode, RustLayer, RustModule,
};
use super::policy::{
    InternalDependencyCapability, classify_rust_sources, rust_dependency_findings,
    rust_dependency_findings_with_capabilities,
};
use super::semantic_guards::{
    PROJECT_WATCHER_BOUNDARY_RULE, PURE_LEAF_GRAPH_DOCUMENT_JSON_RULE, TABULAR_CONTRACT_RULE,
    project_to_graph_production_edges, project_watcher_source_violations,
    pure_leaf_graph_document_json_violations, tabular_contract_source_violations,
};

fn repository_root() -> PathBuf {
    let manifest_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri must have a repository parent")
        .to_path_buf();
    std::fs::canonicalize(manifest_root).expect("repository root must be canonicalizable")
}

fn workspace_facts() -> &'static super::model::RustWorkspaceModel {
    static WORKSPACE: std::sync::OnceLock<super::model::RustWorkspaceModel> =
        std::sync::OnceLock::new();
    WORKSPACE.get_or_init(|| {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        super::cargo_targets::discover_rust_workspace_model(&manifest)
            .expect("the real Cargo workspace must be discoverable")
    })
}

#[test]
fn julia_plugin_depends_only_on_plugin_crates_and_explicit_common_libraries() {
    let workspace = workspace_facts();
    let plugin_root = workspace
        .repository_root
        .join("plugins/julia/native/crates");
    let plugin_packages = workspace
        .roots
        .iter()
        .filter(|root| root.source_path.starts_with(&plugin_root))
        .map(|root| root.package.clone())
        .collect::<BTreeSet<_>>();
    assert!(!plugin_packages.is_empty());
    let common = [
        "yss-plugin-protocol",
        "yss-plugin-sdk",
        "yss-math-expr",
        "yss-sci-contract",
        "yss-file-replace",
    ];
    let mut pending = plugin_packages.iter().cloned().collect::<Vec<_>>();
    let mut visited = BTreeSet::new();
    while let Some(package) = pending.pop() {
        if !visited.insert(package.clone()) {
            continue;
        }
        for dependency in workspace
            .dependency_declarations
            .iter()
            .filter(|dependency| {
                dependency.owning_package == package
                    && dependency.scope != CargoDependencyScope::Development
                    && matches!(
                        dependency.authority,
                        CargoDependencyAuthority::WorkspaceMember { .. }
                    )
            })
        {
            assert!(
                plugin_packages.contains(&dependency.package_name)
                    || common.contains(&dependency.package_name.as_str()),
                "plugin dependency chain reaches an unapproved host crate: {package} -> {}",
                dependency.package_name
            );
            pending.push(dependency.package_name.clone());
        }
    }
}

fn declares_dependency(package: &str, declaration: &str) -> bool {
    let name = declaration.trim().split([' ', '.', '=']).next().unwrap();
    workspace_facts()
        .dependency_declarations
        .iter()
        .any(|dependency| {
            dependency.owning_package == package
                && dependency.scope == CargoDependencyScope::Runtime
                && (dependency.package_name == name
                    || dependency.declared_name == name.replace('-', "_"))
                && (!name.starts_with("yss-")
                    || matches!(
                        dependency.authority,
                        CargoDependencyAuthority::WorkspaceMember { .. }
                    ))
        })
}

fn workspace_declares(declaration: &str) -> bool {
    if declaration.starts_with('"') {
        let package = declaration.trim_matches('"').rsplit('/').next().unwrap();
        workspace_facts()
            .roots
            .iter()
            .any(|root| root.package == package)
    } else {
        declares_dependency("yssbi", declaration)
    }
}

fn declares_dependency_family(package: &str, declaration: &str) -> bool {
    if declaration.contains('=') {
        return declares_dependency(package, declaration);
    }
    let name = declaration.trim().split([' ', '.', '=']).next().unwrap();
    workspace_facts()
        .dependency_declarations
        .iter()
        .any(|dependency| {
            dependency.owning_package == package
                && dependency.scope == CargoDependencyScope::Runtime
                && (dependency.package_name == name
                    || dependency.package_name.starts_with(&format!("{name}-")))
        })
}

struct ProductionFacts {
    repository_root: PathBuf,
    dependencies: Vec<CanonicalDependency>,
    classification: BTreeMap<String, RustLayer>,
}

fn production_facts() -> &'static ProductionFacts {
    static FACTS: std::sync::OnceLock<ProductionFacts> = std::sync::OnceLock::new();
    FACTS.get_or_init(|| {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let workspace = super::cargo_targets::discover_rust_workspace_model(&manifest)
            .expect("the real Cargo workspace must be discoverable");
        let modules = collect_production_modules(&workspace.repository_root, &workspace.roots)
            .expect("the production module graph must be discoverable");
        let raw_dependencies =
            collect_production_dependencies(&workspace.repository_root, &workspace.roots)
                .expect("production dependency facts must be discoverable");
        let dependencies = resolve_canonical_dependencies_detailed(&workspace, &raw_dependencies)
            .expect("production dependencies must resolve");
        let classification = classify_rust_sources(&workspace.roots, &modules)
            .expect("production sources must classify");
        ProductionFacts {
            repository_root: workspace.repository_root,
            dependencies,
            classification,
        }
    })
}

fn source_path(relative: &str) -> String {
    repository_root()
        .join(relative)
        .to_string_lossy()
        .into_owned()
}

fn metadata_fixture_with_all_target_kinds() -> Value {
    let yssbi_id = "path+file:///fixture/src-tauri#yssbi@0.3.0";
    let sci_id = "path+file:///fixture/src-tauri/crates/yss-sci#yss-sci@0.1.0";
    json!({
        "workspace_members": [yssbi_id, sci_id],
        "packages": [
            {
                "id": yssbi_id,
                "name": "yssbi",
                "targets": [
                    {"kind": ["staticlib", "cdylib", "rlib"], "name": "yssbi_lib", "src_path": source_path("src-tauri/src/lib.rs")},
                    {"kind": ["bin"], "name": "yssbi", "src_path": source_path("src-tauri/src/main.rs")},
                    {"kind": ["example"], "name": "architecture_fixture", "src_path": source_path("src-tauri/src/main.rs")},
                    {"kind": ["custom-build"], "name": "build-script-build", "src_path": source_path("src-tauri/build.rs")},
                    {"kind": ["test"], "name": "ignored_test", "src_path": source_path("src-tauri/src/lib.rs")},
                    {"kind": ["bench"], "name": "ignored_bench", "src_path": source_path("src-tauri/src/lib.rs")}
                ],
                "dependencies": [
                    {"name": "yss-sci", "package": "yss-sci", "rename": "science_api", "kind": null, "target": null},
                    {"name": "serde", "package": "serde", "kind": null, "target": null},
                    {"name": "renamed-runtime", "package": "serde_json", "rename": "json_api", "kind": "normal", "target": "cfg(windows)"},
                    {"name": "tauri-build", "package": "tauri-build", "kind": "build", "target": null},
                    {"name": "proc-macro2", "package": "proc-macro2", "kind": "dev", "target": null}
                ]
            },
            {
                "id": sci_id,
                "name": "yss-sci",
                "targets": [
                    {"kind": ["lib"], "name": "yss_sci", "src_path": source_path("src-tauri/crates/yss-sci/src/lib.rs")},
                    {"kind": ["test"], "name": "ignored_sci_test", "src_path": source_path("src-tauri/crates/yss-sci/src/lib.rs")}
                ],
                "dependencies": [
                    {"name": "serde", "package": "serde", "kind": null, "target": null},
                    {"name": "csv", "package": "csv", "kind": "dev", "target": null}
                ]
            }
        ]
    })
}

#[test]
fn production_roots_cover_every_workspace_target() {
    let workspace = rust_workspace_model_from_metadata(
        &repository_root(),
        metadata_fixture_with_all_target_kinds(),
    )
    .expect("valid metadata fixture must decode");

    let roots = workspace
        .roots
        .iter()
        .map(|root| (&root.package, &root.target, root.kind))
        .collect::<Vec<_>>();
    assert_eq!(
        roots,
        vec![
            (
                &"yss-sci".to_owned(),
                &"yss_sci".to_owned(),
                ProductionRootKind::Library
            ),
            (
                &"yssbi".to_owned(),
                &"architecture_fixture".to_owned(),
                ProductionRootKind::Example
            ),
            (
                &"yssbi".to_owned(),
                &"build-script-build".to_owned(),
                ProductionRootKind::BuildScript
            ),
            (
                &"yssbi".to_owned(),
                &"yssbi".to_owned(),
                ProductionRootKind::Binary
            ),
            (
                &"yssbi".to_owned(),
                &"yssbi_lib".to_owned(),
                ProductionRootKind::Library
            ),
        ]
    );
    assert!(
        workspace
            .roots
            .iter()
            .all(|root| root.source_path.starts_with(repository_root()))
    );

    let science_alias = workspace
        .workspace_member_crate_aliases
        .iter()
        .find(|alias| alias.owning_package == "yssbi" && alias.declared_name == "science_api")
        .expect("the fixture must preserve the renamed SCI workspace member");
    assert_eq!(science_alias.member_package, "yss-sci");
    assert_eq!(science_alias.library_crate_name, "yss_sci");
    assert_eq!(science_alias.root_owner, "yss_sci");
    assert_eq!(
        science_alias.library_root,
        std::fs::canonicalize(repository_root().join("src-tauri/crates/yss-sci/src/lib.rs"))
            .expect("SCI library root must exist")
    );

    let science_dependency = workspace
        .dependency_declarations
        .iter()
        .find(|dependency| {
            dependency.owning_package == "yssbi" && dependency.declared_name == "science_api"
        })
        .expect("the fixture must preserve the SCI dependency declaration");
    assert_eq!(science_dependency.package_name, "yss-sci");
    assert_eq!(science_dependency.scope, CargoDependencyScope::Runtime);
    assert_eq!(
        science_dependency.authority,
        CargoDependencyAuthority::WorkspaceMember {
            member_package_id: "path+file:///fixture/src-tauri/crates/yss-sci#yss-sci@0.1.0"
                .to_owned()
        }
    );

    let build_dependency = workspace
        .dependency_declarations
        .iter()
        .find(|dependency| dependency.declared_name == "tauri_build")
        .expect("the fixture must preserve the build dependency");
    assert_eq!(build_dependency.scope, CargoDependencyScope::Build);
    assert_eq!(build_dependency.target_condition, None);

    let renamed_dependency = workspace
        .dependency_declarations
        .iter()
        .find(|dependency| dependency.declared_name == "json_api")
        .expect("the fixture must preserve the renamed runtime dependency");
    assert_eq!(renamed_dependency.package_name, "serde_json");
    assert_eq!(
        renamed_dependency.target_condition.as_deref(),
        Some("cfg(windows)")
    );

    let development_dependency = workspace
        .dependency_declarations
        .iter()
        .find(|dependency| dependency.declared_name == "proc_macro2")
        .expect("the fixture must retain development declarations for policy checks");
    assert_eq!(
        development_dependency.scope,
        CargoDependencyScope::Development
    );
}

#[test]
fn real_workspace_discovery_includes_production_targets_and_member_alias() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let workspace = super::cargo_targets::discover_rust_workspace_model(&manifest)
        .expect("the real Cargo workspace must be discoverable");

    assert!(workspace.roots.iter().any(|root| root.package == "yssbi"
        && root.target == "yssbi_lib"
        && root.kind == ProductionRootKind::Library));
    assert!(workspace.roots.iter().any(|root| root.package == "yssbi"
        && root.target == "yssbi"
        && root.kind == ProductionRootKind::Binary));
    assert!(workspace.roots.iter().any(|root| root.package == "yss-sci"
        && root.target == "yss_sci"
        && root.kind == ProductionRootKind::Library));
    assert!(
        workspace
            .roots
            .iter()
            .any(|root| root.package == "yss-canonical-hash"
                && root.target == "yss_canonical_hash"
                && root.kind == ProductionRootKind::Library)
    );
    assert!(
        workspace
            .roots
            .iter()
            .any(|root| root.package == "yss-project-model"
                && root.target == "yss_project_model"
                && root.kind == ProductionRootKind::Library)
    );
    assert!(
        workspace
            .roots
            .iter()
            .any(|root| root.package == "yss-data-contract"
                && root.target == "yss_data_contract"
                && root.kind == ProductionRootKind::Library)
    );
    assert!(
        workspace
            .roots
            .iter()
            .any(|root| root.package == "yss-database-contract"
                && root.target == "yss_database_contract"
                && root.kind == ProductionRootKind::Library)
    );
    assert!(
        workspace
            .roots
            .iter()
            .any(|root| root.package == "yss-diagnostics"
                && root.target == "yss_diagnostics"
                && root.kind == ProductionRootKind::Library)
    );
    assert!(
        workspace
            .roots
            .iter()
            .any(|root| root.package == "yss-execution"
                && root.target == "yss_execution"
                && root.kind == ProductionRootKind::Library)
    );
    assert!(
        workspace
            .roots
            .iter()
            .any(|root| root.package == "yss-graph-analysis"
                && root.target == "yss_graph_analysis"
                && root.kind == ProductionRootKind::Library)
    );
    assert!(
        workspace
            .roots
            .iter()
            .any(|root| root.package == "yss-graph-analysis-contract"
                && root.target == "yss_graph_analysis_contract"
                && root.kind == ProductionRootKind::Library)
    );
    assert!(
        workspace
            .roots
            .iter()
            .any(|root| root.package == "yss-graph-catalog"
                && root.target == "yss_graph_catalog"
                && root.kind == ProductionRootKind::Library)
    );
    assert!(
        workspace
            .roots
            .iter()
            .any(|root| root.package == "yss-graph-compiler"
                && root.target == "yss_graph_compiler"
                && root.kind == ProductionRootKind::Library)
    );
    assert!(
        workspace
            .roots
            .iter()
            .any(|root| root.package == "yss-graph-compiler-diagnostics"
                && root.target == "yss_graph_compiler_diagnostics"
                && root.kind == ProductionRootKind::Library)
    );
    assert!(
        workspace
            .roots
            .iter()
            .any(|root| root.package == "yss-graph-document"
                && root.target == "yss_graph_document"
                && root.kind == ProductionRootKind::Library)
    );
    assert!(
        workspace
            .roots
            .iter()
            .any(|root| root.package == "yss-graph-document-edit"
                && root.target == "yss_graph_document_edit"
                && root.kind == ProductionRootKind::Library)
    );
    assert!(
        workspace
            .roots
            .iter()
            .any(|root| root.package == "yss-graph-protocol"
                && root.target == "yss_graph_protocol"
                && root.kind == ProductionRootKind::Library)
    );
    assert!(
        workspace
            .roots
            .iter()
            .any(|root| root.package == "yss-graph-resource-contract"
                && root.target == "yss_graph_resource_contract"
                && root.kind == ProductionRootKind::Library)
    );
    assert!(
        workspace
            .roots
            .iter()
            .any(|root| root.package == "yss-graph-type-mapping"
                && root.target == "yss_graph_type_mapping"
                && root.kind == ProductionRootKind::Library)
    );
    assert!(
        workspace
            .roots
            .iter()
            .any(|root| root.package == "yss-graph-registry"
                && root.target == "yss_graph_registry"
                && root.kind == ProductionRootKind::Library)
    );
    assert!(
        workspace
            .roots
            .iter()
            .any(|root| root.package == "yss-math-expr"
                && root.target == "yss_math_expr"
                && root.kind == ProductionRootKind::Library)
    );
    assert!(
        workspace
            .roots
            .iter()
            .any(|root| root.package == "yss-path-display"
                && root.target == "yss_path_display"
                && root.kind == ProductionRootKind::Library)
    );
    assert!(
        workspace
            .roots
            .iter()
            .any(|root| root.package == "yss-tabular-contract"
                && root.target == "yss_tabular_contract"
                && root.kind == ProductionRootKind::Library)
    );
    assert!(
        workspace
            .roots
            .iter()
            .any(|root| root.package == "yss-window-state"
                && root.target == "yss_window_state"
                && root.kind == ProductionRootKind::Library)
    );
    assert!(
        workspace
            .roots
            .iter()
            .any(|root| root.package == "yss-tracing"
                && root.target == "yss_tracing"
                && root.kind == ProductionRootKind::Library)
    );
    assert!(workspace.dependency_declarations.iter().any(|dependency| {
        dependency.owning_package == "yssbi"
            && dependency.declared_name == "tauri_build"
            && dependency.scope == CargoDependencyScope::Build
    }));
    assert!(
        workspace
            .workspace_member_crate_aliases
            .iter()
            .any(|alias| {
                alias.owning_package == "yss-api"
                    && alias.declared_name == "yss_sci_runtime"
                    && alias.member_package == "yss-sci-runtime"
            })
    );
    assert!(workspace.dependency_declarations.iter().any(|dependency| {
        dependency.owning_package == "yss-api"
            && dependency.package_name == "yss-sci-runtime"
            && matches!(
                dependency.authority,
                CargoDependencyAuthority::WorkspaceMember { .. }
            )
    }));
    for (owning_package, declared_name, member_package) in [
        ("yssbi", "yss_application", "yss-application"),
        ("yss-api", "yss_data_contract", "yss-data-contract"),
        (
            "yss-application",
            "yss_graph_compiler",
            "yss-graph-compiler",
        ),
        (
            "yss-graph-catalog",
            "yss_graph_compiler_diagnostics",
            "yss-graph-compiler-diagnostics",
        ),
        (
            "yss-graph-editor",
            "yss_graph_type_mapping",
            "yss-graph-type-mapping",
        ),
    ] {
        assert!(
            workspace
                .workspace_member_crate_aliases
                .iter()
                .any(|alias| {
                    alias.owning_package == owning_package
                        && alias.declared_name == declared_name
                        && alias.member_package == member_package
                })
        );
        assert!(workspace.dependency_declarations.iter().any(|dependency| {
            dependency.owning_package == owning_package
                && dependency.package_name == member_package
                && matches!(
                    dependency.authority,
                    CargoDependencyAuthority::WorkspaceMember { .. }
                )
        }));
    }
    assert!(
        workspace
            .roots
            .iter()
            .all(|root| root.source_path.starts_with(&workspace.repository_root))
    );
}

#[test]
fn bayes_artifact_contract_and_datafusion_adapter_have_distinct_acyclic_owners() {
    let facts = production_facts();
    const CONTRACT_PREFIX: &str = "plugins/julia/native/crates/yss-bayes-artifact-contract/src/";
    const ADAPTER_SOURCE: &str =
        "plugins/julia/native/crates/yss-bayes-artifact-datafusion/src/lib.rs";

    for relative in [
        "plugins/julia/native/crates/yss-bayes-artifact-contract/Cargo.toml",
        "plugins/julia/native/crates/yss-bayes-artifact-contract/README.md",
        "plugins/julia/native/crates/yss-bayes-artifact-contract/src/lib.rs",
        "plugins/julia/native/crates/yss-bayes-artifact-datafusion/Cargo.toml",
        "plugins/julia/native/crates/yss-bayes-artifact-datafusion/README.md",
        ADAPTER_SOURCE,
    ] {
        assert!(
            facts.repository_root.join(relative).is_file(),
            "Bayes artifact contract boundary must exist at {relative}"
        );
    }

    assert!(declares_dependency(
        "yss-bayes-artifact-contract",
        "yss-bayes-result"
    ));
    for backwards_dependency in ["polars", "tauri", "yss-application", "yss-tabular-io"] {
        assert!(
            !declares_dependency_family("yss-bayes-artifact-contract", backwards_dependency),
            "Bayes artifact contract must not depend on {backwards_dependency}"
        );
    }

    for dependency in [
        "arrow.workspace = true",
        "datafusion.workspace = true",
        "yss-bayes-artifact-contract",
        "yss-bayes-result",
    ] {
        assert!(
            declares_dependency("yss-bayes-artifact-datafusion", dependency),
            "Bayes artifact DataFusion adapter must declare {dependency}"
        );
    }
    for backwards_dependency in [
        "tauri",
        "yss-application",
        "yssbi",
        "yss-sci-runtime",
        "yss-tabular-io",
    ] {
        assert!(
            !declares_dependency_family("yss-bayes-artifact-datafusion", backwards_dependency),
            "Bayes artifact DataFusion adapter must not depend on {backwards_dependency}"
        );
    }
    assert!(
        !facts
            .repository_root
            .join("src-tauri/src/backend_adapters/execution/bayes_artifacts.rs")
            .exists(),
        "the root Bayes artifact adapter owner must stay absent"
    );

    let application = std::fs::read_to_string(
        facts
            .repository_root
            .join("plugins/julia/native/crates/yss-bayes-runtime/src/lib.rs"),
    )
    .expect("Bayes application source must be readable");
    let adapter = std::fs::read_to_string(
        facts
            .repository_root
            .join("plugins/julia/native/crates/yss-bayes-artifact-datafusion/src/lib.rs"),
    )
    .expect("Bayes artifact adapter must be readable");
    for consumer in [&application, &adapter] {
        assert!(
            consumer.contains("yss_bayes_artifact_contract"),
            "Application and backend adapter must consume the canonical artifact contract"
        );
    }
    for former_owner in [
        "pub enum BayesArtifactReadError",
        "pub trait BayesArtifactReader",
        "fn posterior_sample_page_from_dataframe",
        "fn posterior_predictive_page_from_dataframe",
        "fn trace_plot_data_from_dataframe",
        "fn density_plot_data_from_dataframe",
        "fn autocorrelation_plot_data_from_dataframe",
    ] {
        assert!(
            !application.contains(former_owner),
            "Application must not retain duplicate artifact contract {former_owner}"
        );
    }
    assert!(
        !application.contains("polars::"),
        "Bayes Application source must not retain Polars-backed adapter or test-mirror logic"
    );
    let extension = std::fs::read_to_string(
        facts
            .repository_root
            .join("plugins/julia/native/crates/yss-julia-extension/src/main.rs"),
    )
    .expect("plugin composition must be readable");
    assert!(
        extension.contains("yss_bayes_artifact_datafusion::DataFusionBayesArtifactReader::new()")
    );

    let contract_sources = facts
        .classification
        .iter()
        .filter(|(source, _)| source.starts_with(CONTRACT_PREFIX))
        .collect::<Vec<_>>();
    assert!(!contract_sources.is_empty());
    assert!(
        contract_sources
            .iter()
            .all(|(_, layer)| **layer == RustLayer::PureLeaf),
        "every yss-bayes-artifact-contract production source must remain a Pure Leaf"
    );
    assert_eq!(
        facts.classification.get(ADAPTER_SOURCE),
        Some(&RustLayer::BackendAdapter),
        "the concrete DataFusion reader must remain a Backend Adapter"
    );
}

#[test]
fn production_target_discovery_fails_closed_for_unknown_kind() {
    let mut metadata = metadata_fixture_with_all_target_kinds();
    metadata["packages"][0]["targets"]
        .as_array_mut()
        .expect("fixture targets must be an array")
        .push(json!({
            "kind": ["future-production-kind"],
            "name": "future_target",
            "src_path": source_path("src-tauri/src/lib.rs")
        }));

    let error = rust_workspace_model_from_metadata(&repository_root(), metadata)
        .expect_err("unknown production target kinds must fail closed");
    assert!(matches!(
        error,
        ArchitectureAuditError::InvalidMetadata { .. }
    ));
}

#[test]
fn rust_layer_classifier_is_total_and_exclusive() {
    let runtime_root = ProductionRoot {
        package_id: "fixture-package".to_owned(),
        package: "fixture".to_owned(),
        target: "fixture_lib".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/src/lib.rs"),
    };
    let application_root = ProductionRoot {
        package_id: "application-package".to_owned(),
        package: "yss-application".to_owned(),
        target: "yss_application".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-application/src/lib.rs"),
    };
    let data_contract_root = ProductionRoot {
        package_id: "data-contract-package".to_owned(),
        package: "yss-data-contract".to_owned(),
        target: "yss_data_contract".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-data-contract/src/lib.rs"),
    };
    let canonical_hash_root = ProductionRoot {
        package_id: "canonical-hash-package".to_owned(),
        package: "yss-canonical-hash".to_owned(),
        target: "yss_canonical_hash".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-canonical-hash/src/lib.rs"),
    };
    let database_contract_root = ProductionRoot {
        package_id: "database-contract-package".to_owned(),
        package: "yss-database-contract".to_owned(),
        target: "yss_database_contract".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-database-contract/src/lib.rs"),
    };
    let diagnostics_root = ProductionRoot {
        package_id: "diagnostics-package".to_owned(),
        package: "yss-diagnostics".to_owned(),
        target: "yss_diagnostics".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-diagnostics/src/lib.rs"),
    };
    let execution_root = ProductionRoot {
        package_id: "execution-package".to_owned(),
        package: "yss-execution".to_owned(),
        target: "yss_execution".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-execution/src/lib.rs"),
    };
    let graph_analysis_root = ProductionRoot {
        package_id: "graph-analysis-package".to_owned(),
        package: "yss-graph-analysis".to_owned(),
        target: "yss_graph_analysis".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-graph-analysis/src/lib.rs"),
    };
    let graph_analysis_contract_root = ProductionRoot {
        package_id: "graph-analysis-contract-package".to_owned(),
        package: "yss-graph-analysis-contract".to_owned(),
        target: "yss_graph_analysis_contract".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-graph-analysis-contract/src/lib.rs"),
    };
    let graph_catalog_root = ProductionRoot {
        package_id: "graph-catalog-package".to_owned(),
        package: "yss-graph-catalog".to_owned(),
        target: "yss_graph_catalog".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-graph-catalog/src/lib.rs"),
    };
    let graph_compiler_root = ProductionRoot {
        package_id: "graph-compiler-package".to_owned(),
        package: "yss-graph-compiler".to_owned(),
        target: "yss_graph_compiler".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-graph-compiler/src/lib.rs"),
    };
    let graph_compiler_diagnostics_root = ProductionRoot {
        package_id: "graph-compiler-diagnostics-package".to_owned(),
        package: "yss-graph-compiler-diagnostics".to_owned(),
        target: "yss_graph_compiler_diagnostics".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-graph-compiler-diagnostics/src/lib.rs"),
    };
    let graph_document_root = ProductionRoot {
        package_id: "graph-document-package".to_owned(),
        package: "yss-graph-document".to_owned(),
        target: "yss_graph_document".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-graph-document/src/lib.rs"),
    };
    let graph_document_edit_root = ProductionRoot {
        package_id: "graph-document-edit-package".to_owned(),
        package: "yss-graph-document-edit".to_owned(),
        target: "yss_graph_document_edit".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-graph-document-edit/src/lib.rs"),
    };
    let graph_protocol_root = ProductionRoot {
        package_id: "graph-protocol-package".to_owned(),
        package: "yss-graph-protocol".to_owned(),
        target: "yss_graph_protocol".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-graph-protocol/src/lib.rs"),
    };
    let graph_resource_contract_root = ProductionRoot {
        package_id: "graph-resource-contract-package".to_owned(),
        package: "yss-graph-resource-contract".to_owned(),
        target: "yss_graph_resource_contract".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-graph-resource-contract/src/lib.rs"),
    };
    let graph_type_mapping_root = ProductionRoot {
        package_id: "graph-type-mapping-package".to_owned(),
        package: "yss-graph-type-mapping".to_owned(),
        target: "yss_graph_type_mapping".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-graph-type-mapping/src/lib.rs"),
    };
    let graph_registry_root = ProductionRoot {
        package_id: "graph-registry-package".to_owned(),
        package: "yss-graph-registry".to_owned(),
        target: "yss_graph_registry".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-graph-registry/src/lib.rs"),
    };
    let math_root = ProductionRoot {
        package_id: "math-package".to_owned(),
        package: "yss-math-expr".to_owned(),
        target: "yss_math_expr".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-math-expr/src/lib.rs"),
    };
    let path_display_root = ProductionRoot {
        package_id: "path-display-package".to_owned(),
        package: "yss-path-display".to_owned(),
        target: "yss_path_display".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-path-display/src/lib.rs"),
    };
    let project_discovery_root = ProductionRoot {
        package_id: "project-discovery-package".to_owned(),
        package: "yss-project-discovery".to_owned(),
        target: "yss_project_discovery".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-project-discovery/src/lib.rs"),
    };
    let project_history_root = ProductionRoot {
        package_id: "project-history-package".to_owned(),
        package: "yss-project-history".to_owned(),
        target: "yss_project_history".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-project-history/src/lib.rs"),
    };
    let project_manifest_root = ProductionRoot {
        package_id: "project-manifest-package".to_owned(),
        package: "yss-project-identity".to_owned(),
        target: "yss_project_identity".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-project-identity/src/lib.rs"),
    };
    let project_model_root = ProductionRoot {
        package_id: "project-model-package".to_owned(),
        package: "yss-project-model".to_owned(),
        target: "yss_project_model".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-project-model/src/lib.rs"),
    };
    let tabular_contract_root = ProductionRoot {
        package_id: "tabular-contract-package".to_owned(),
        package: "yss-tabular-contract".to_owned(),
        target: "yss_tabular_contract".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-tabular-contract/src/lib.rs"),
    };
    let window_state_root = ProductionRoot {
        package_id: "window-state-package".to_owned(),
        package: "yss-window-state".to_owned(),
        target: "yss_window_state".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-window-state/src/lib.rs"),
    };
    let build_root = ProductionRoot {
        package_id: "fixture-package".to_owned(),
        package: "fixture".to_owned(),
        target: "build-script-build".to_owned(),
        kind: ProductionRootKind::BuildScript,
        source_path: PathBuf::from("src-tauri/build.rs"),
    };
    let roots = vec![
        runtime_root.clone(),
        application_root.clone(),
        canonical_hash_root.clone(),
        data_contract_root.clone(),
        database_contract_root.clone(),
        diagnostics_root.clone(),
        execution_root.clone(),
        graph_analysis_root.clone(),
        graph_analysis_contract_root.clone(),
        graph_catalog_root.clone(),
        graph_compiler_root.clone(),
        graph_compiler_diagnostics_root.clone(),
        graph_document_root.clone(),
        graph_document_edit_root.clone(),
        graph_protocol_root.clone(),
        graph_resource_contract_root.clone(),
        graph_type_mapping_root.clone(),
        graph_registry_root.clone(),
        math_root.clone(),
        path_display_root.clone(),
        project_discovery_root.clone(),
        project_history_root.clone(),
        project_manifest_root.clone(),
        project_model_root.clone(),
        tabular_contract_root.clone(),
        window_state_root.clone(),
        build_root.clone(),
    ];
    let module = |root: &ProductionRoot, source_file: &str, owner: &str| RustModule {
        root_package_id: root.package_id.clone(),
        root_target: root.target.clone(),
        root_kind: root.kind,
        repository_relative_source_file: source_file.to_owned(),
        fully_qualified_owner: owner.to_owned(),
    };

    let classified = classify_rust_sources(
        &roots,
        &[
            module(
                &application_root,
                "src-tauri/crates/yss-application/src/lib.rs",
                "yss_application",
            ),
            module(
                &graph_catalog_root,
                "src-tauri/crates/yss-graph-catalog/src/lib.rs",
                "yss_graph_catalog",
            ),
            module(
                &graph_catalog_root,
                "src-tauri/crates/yss-graph-catalog/src/builtin.rs",
                "yss_graph_catalog::builtin",
            ),
            module(
                &canonical_hash_root,
                "src-tauri/crates/yss-canonical-hash/src/lib.rs",
                "yss_canonical_hash",
            ),
            module(
                &data_contract_root,
                "src-tauri/crates/yss-data-contract/src/lib.rs",
                "yss_data_contract",
            ),
            module(
                &data_contract_root,
                "src-tauri/crates/yss-data-contract/src/data_type.rs",
                "yss_data_contract::data_type",
            ),
            module(
                &data_contract_root,
                "src-tauri/crates/yss-data-contract/src/data_value.rs",
                "yss_data_contract::data_value",
            ),
            module(
                &database_contract_root,
                "src-tauri/crates/yss-database-contract/src/lib.rs",
                "yss_database_contract",
            ),
            module(
                &diagnostics_root,
                "src-tauri/crates/yss-diagnostics/src/lib.rs",
                "yss_diagnostics",
            ),
            module(
                &graph_analysis_root,
                "src-tauri/crates/yss-graph-analysis/src/lib.rs",
                "yss_graph_analysis",
            ),
            module(
                &graph_analysis_contract_root,
                "src-tauri/crates/yss-graph-analysis-contract/src/lib.rs",
                "yss_graph_analysis_contract",
            ),
            module(
                &graph_compiler_root,
                "src-tauri/crates/yss-graph-compiler/src/lib.rs",
                "yss_graph_compiler",
            ),
            module(
                &graph_compiler_diagnostics_root,
                "src-tauri/crates/yss-graph-compiler-diagnostics/src/lib.rs",
                "yss_graph_compiler_diagnostics",
            ),
            module(
                &graph_document_root,
                "src-tauri/crates/yss-graph-document/src/lib.rs",
                "yss_graph_document",
            ),
            module(
                &graph_document_edit_root,
                "src-tauri/crates/yss-graph-document-edit/src/lib.rs",
                "yss_graph_document_edit",
            ),
            module(
                &graph_protocol_root,
                "src-tauri/crates/yss-graph-protocol/src/lib.rs",
                "yss_graph_protocol",
            ),
            module(
                &graph_resource_contract_root,
                "src-tauri/crates/yss-graph-resource-contract/src/lib.rs",
                "yss_graph_resource_contract",
            ),
            module(
                &graph_type_mapping_root,
                "src-tauri/crates/yss-graph-type-mapping/src/lib.rs",
                "yss_graph_type_mapping",
            ),
            module(
                &graph_registry_root,
                "src-tauri/crates/yss-graph-registry/src/lib.rs",
                "yss_graph_registry",
            ),
            module(
                &math_root,
                "src-tauri/crates/yss-math-expr/src/lib.rs",
                "yss_math_expr",
            ),
            module(
                &path_display_root,
                "src-tauri/crates/yss-path-display/src/lib.rs",
                "yss_path_display",
            ),
            module(
                &project_discovery_root,
                "src-tauri/crates/yss-project-discovery/src/lib.rs",
                "yss_project_discovery",
            ),
            module(
                &project_history_root,
                "src-tauri/crates/yss-project-history/src/lib.rs",
                "yss_project_history",
            ),
            module(
                &project_manifest_root,
                "src-tauri/crates/yss-project-identity/src/lib.rs",
                "yss_project_identity",
            ),
            module(
                &project_model_root,
                "src-tauri/crates/yss-project-model/src/lib.rs",
                "yss_project_model",
            ),
            module(
                &tabular_contract_root,
                "src-tauri/crates/yss-tabular-contract/src/lib.rs",
                "yss_tabular_contract",
            ),
            module(
                &window_state_root,
                "src-tauri/crates/yss-window-state/src/lib.rs",
                "yss_window_state",
            ),
            module(
                &execution_root,
                "src-tauri/crates/yss-execution/src/state.rs",
                "yss_execution::state",
            ),
            module(&build_root, "src-tauri/build.rs", "build_script_build"),
            module(
                &build_root,
                "src-tauri/build_support.rs",
                "build_script_build::build_support",
            ),
        ],
    )
    .expect("the representative production files must classify exactly once");
    assert_eq!(
        classified["src-tauri/crates/yss-application/src/lib.rs"],
        RustLayer::Application
    );
    assert_eq!(
        classified["src-tauri/crates/yss-graph-catalog/src/builtin.rs"],
        RustLayer::BuiltinComposition
    );
    assert_eq!(
        classified["src-tauri/crates/yss-graph-catalog/src/lib.rs"],
        RustLayer::Graph
    );
    assert_eq!(
        classified["src-tauri/crates/yss-canonical-hash/src/lib.rs"],
        RustLayer::PureLeaf
    );
    assert_eq!(
        classified["src-tauri/crates/yss-data-contract/src/lib.rs"],
        RustLayer::PureLeaf
    );
    assert_eq!(
        classified["src-tauri/crates/yss-data-contract/src/data_type.rs"],
        RustLayer::PureLeaf
    );
    assert_eq!(
        classified["src-tauri/crates/yss-data-contract/src/data_value.rs"],
        RustLayer::PureLeaf
    );
    assert_eq!(
        classified["src-tauri/crates/yss-database-contract/src/lib.rs"],
        RustLayer::PureLeaf
    );
    assert_eq!(
        classified["src-tauri/crates/yss-diagnostics/src/lib.rs"],
        RustLayer::Diagnostics
    );
    assert_eq!(
        classified["src-tauri/crates/yss-graph-analysis/src/lib.rs"],
        RustLayer::Graph
    );
    assert_eq!(
        classified["src-tauri/crates/yss-graph-analysis-contract/src/lib.rs"],
        RustLayer::Graph
    );
    assert_eq!(
        classified["src-tauri/crates/yss-graph-compiler/src/lib.rs"],
        RustLayer::Graph
    );
    assert_eq!(
        classified["src-tauri/crates/yss-graph-compiler-diagnostics/src/lib.rs"],
        RustLayer::Graph
    );
    assert_eq!(
        classified["src-tauri/crates/yss-graph-document/src/lib.rs"],
        RustLayer::PureLeaf
    );
    assert_eq!(
        classified["src-tauri/crates/yss-graph-document-edit/src/lib.rs"],
        RustLayer::Graph
    );
    assert_eq!(
        classified["src-tauri/crates/yss-graph-protocol/src/lib.rs"],
        RustLayer::PureLeaf
    );
    assert_eq!(
        classified["src-tauri/crates/yss-graph-resource-contract/src/lib.rs"],
        RustLayer::PureLeaf
    );
    assert_eq!(
        classified["src-tauri/crates/yss-graph-type-mapping/src/lib.rs"],
        RustLayer::PureLeaf
    );
    assert_eq!(
        classified["src-tauri/crates/yss-graph-registry/src/lib.rs"],
        RustLayer::Graph
    );
    assert_eq!(
        classified["src-tauri/crates/yss-math-expr/src/lib.rs"],
        RustLayer::PureLeaf
    );
    assert_eq!(
        classified["src-tauri/crates/yss-path-display/src/lib.rs"],
        RustLayer::PureLeaf
    );
    assert_eq!(
        classified["src-tauri/crates/yss-project-discovery/src/lib.rs"],
        RustLayer::Project
    );
    assert_eq!(
        classified["src-tauri/crates/yss-project-history/src/lib.rs"],
        RustLayer::Project
    );
    assert_eq!(
        classified["src-tauri/crates/yss-project-identity/src/lib.rs"],
        RustLayer::PureLeaf
    );
    assert_eq!(
        classified["src-tauri/crates/yss-project-model/src/lib.rs"],
        RustLayer::Project
    );
    assert_eq!(
        classified["src-tauri/crates/yss-tabular-contract/src/lib.rs"],
        RustLayer::PureLeaf
    );
    assert_eq!(
        classified["src-tauri/crates/yss-window-state/src/lib.rs"],
        RustLayer::PlatformAdapter
    );
    assert_eq!(
        classified["src-tauri/crates/yss-execution/src/state.rs"],
        RustLayer::Execution
    );
    assert_eq!(classified["src-tauri/build.rs"], RustLayer::BuildScript);
    assert_eq!(
        classified["src-tauri/build_support.rs"],
        RustLayer::BuildScript
    );

    let unclassified = classify_rust_sources(
        &roots,
        &[module(
            &runtime_root,
            "src-tauri/src/future_owner.rs",
            "fixture_lib::future_owner",
        )],
    )
    .expect_err("a new production owner must fail closed until classified");
    assert!(matches!(
        unclassified,
        ArchitectureAuditError::UnclassifiedProductionSource { source_files }
            if source_files == vec!["src-tauri/src/future_owner.rs"]
    ));

    let overlap = classify_rust_sources(
        &roots,
        &[module(
            &build_root,
            "src-tauri/crates/yss-application/src/execution/session_factory.rs",
            "yss_application::execution::session_factory",
        )],
    )
    .expect_err("BuildScript membership must not hide a second layer match");
    assert!(matches!(
        overlap,
        ArchitectureAuditError::MultiplyClassifiedProductionSource { source_files }
            if source_files
                == vec!["src-tauri/crates/yss-application/src/execution/session_factory.rs"]
    ));
}

#[test]
fn rust_production_sources_are_classified_once() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let workspace = super::cargo_targets::discover_rust_workspace_model(&manifest)
        .expect("the real Cargo workspace must be discoverable");
    let modules = collect_production_modules(&workspace.repository_root, &workspace.roots)
        .expect("the real production module graph must be discoverable");

    let classification = classify_rust_sources(&workspace.roots, &modules)
        .expect("every real production source must classify exactly once");
    let discovered_sources = modules
        .iter()
        .map(|module| module.repository_relative_source_file.as_str())
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(classification.len(), discovered_sources.len());
    assert!(
        RustLayer::ALL
            .iter()
            .all(|layer| classification.values().any(|actual| actual == layer)),
        "the real production graph must exercise all sixteen Rust layers"
    );
}

#[derive(Debug)]
struct CanonicalOwnerExpectation {
    symbol: &'static str,
    required_origin: &'static str,
    allowed_origins: &'static [&'static str],
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct PersistedContractTypeAlias {
    source_file: String,
    alias: String,
    target: String,
}

fn forbidden_persisted_contract_type_aliases(
    repository_root: &Path,
    modules: &[RustModule],
) -> Result<Vec<PersistedContractTypeAlias>, ArchitectureAuditError> {
    let source_files = modules
        .iter()
        .map(|module| module.repository_relative_source_file.as_str())
        .filter(|source_file| forbidden_alias_symbols(source_file).is_some())
        .collect::<BTreeSet<_>>();
    let mut aliases = Vec::new();

    for source_file in source_files {
        let path = repository_root.join(source_file);
        let source =
            std::fs::read_to_string(&path).map_err(|source| ArchitectureAuditError::Io {
                path: path.clone(),
                source,
            })?;
        let syntax =
            syn::parse_file(&source).map_err(|source| ArchitectureAuditError::SourceParse {
                path: path.clone(),
                source,
            })?;
        collect_forbidden_type_aliases(source_file, &syntax.items, &mut aliases);
    }

    aliases.sort();
    Ok(aliases)
}

fn collect_forbidden_type_aliases(
    source_file: &str,
    items: &[Item],
    aliases: &mut Vec<PersistedContractTypeAlias>,
) {
    let Some(forbidden_symbols) = forbidden_alias_symbols(source_file) else {
        return;
    };

    for item in items {
        match item {
            Item::Type(item_type)
                if !crate::test_support::source_audit::is_test_only(&item_type.attrs) =>
            {
                let Some((symbol, target)) = canonical_persisted_contract_path(&item_type.ty)
                else {
                    continue;
                };
                if forbidden_symbols.contains(&symbol.as_str()) {
                    aliases.push(PersistedContractTypeAlias {
                        source_file: source_file.to_owned(),
                        alias: item_type.ident.to_string(),
                        target,
                    });
                }
            }
            Item::Mod(item_mod)
                if !crate::test_support::source_audit::is_test_only(&item_mod.attrs) =>
            {
                if let Some((_, nested_items)) = &item_mod.content {
                    collect_forbidden_type_aliases(source_file, nested_items, aliases);
                }
            }
            _ => {}
        }
    }
}

fn forbidden_alias_symbols(source_file: &str) -> Option<&'static [&'static str]> {
    const PERSISTED_SYMBOLS: &[&str] = &[
        "CategoricalRole",
        "DataSeriesValue",
        "DataType",
        "DataValue",
        "DummyInfo",
        "TimeSeriesState",
    ];
    const SCI_SYMBOLS: &[&str] = &["CategoricalRole"];

    if source_file.starts_with("src-tauri/src/graph/") {
        Some(PERSISTED_SYMBOLS)
    } else if source_file.starts_with("src-tauri/crates/yss-sci-runtime/src/")
        || source_file.starts_with("plugins/julia/native/crates/yss-bayes-model/src/")
        || source_file.starts_with("plugins/julia/native/crates/yss-bayes-result/src/")
        || source_file.starts_with("plugins/julia/native/crates/yss-bayes-worker/src/")
        || source_file.starts_with("src-tauri/crates/yss-sci-contract/src/")
    {
        Some(SCI_SYMBOLS)
    } else {
        None
    }
}

fn canonical_persisted_contract_path(ty: &Type) -> Option<(String, String)> {
    let type_path = match ty {
        Type::Group(group) => return canonical_persisted_contract_path(&group.elem),
        Type::Paren(paren) => return canonical_persisted_contract_path(&paren.elem),
        Type::Path(type_path) if type_path.qself.is_none() => type_path,
        _ => return None,
    };
    let segments = type_path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    let [crate_root, symbol] = segments.as_slice() else {
        return None;
    };
    if type_path.path.leading_colon.is_some()
        || crate_root != "yss_data_contract"
        || type_path
            .path
            .segments
            .iter()
            .any(|segment| !matches!(segment.arguments, syn::PathArguments::None))
    {
        return None;
    }

    Some((symbol.clone(), segments.join("::")))
}

fn canonical_owner_origins_are_valid(
    expectation: &CanonicalOwnerExpectation,
    actual_origins: &BTreeSet<&str>,
) -> bool {
    let allowed_origins = expectation
        .allowed_origins
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();

    actual_origins.contains(expectation.required_origin)
        && actual_origins.is_subset(&allowed_origins)
}

#[test]
fn rust_pure_leaf_graph_document_json_is_serialization_only() {
    let facts = production_facts();
    assert_eq!(
        pure_leaf_graph_document_json_violations(
            &facts.repository_root,
            &facts.dependencies,
            &facts.classification,
        ),
        Vec::new()
    );
}

#[test]
fn rust_pure_leaf_json_guard_scopes_constant_literal_decoding() {
    const PREFIX: &str = "yssbi-pure-leaf-json-guard-";

    struct Fixture {
        root: PathBuf,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            if self
                .root
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(PREFIX))
            {
                let _ = std::fs::remove_dir_all(&self.root);
            }
        }
    }

    let fixture = Fixture {
        root: repository_root()
            .join("target")
            .join(format!("{PREFIX}{}", uuid::Uuid::new_v4())),
    };
    let graph_document = fixture.root.join("src-tauri/crates/yss-graph-document/src");
    std::fs::create_dir_all(&graph_document).expect("fixture graph_document must be created");
    std::fs::write(
        graph_document.join("lib.rs"),
        "mod model;\nmod constant_value;\n#[cfg(test)] mod tests {}\npub type RuntimeEscape = serde_json::Value;\n",
    )
    .expect("fixture graph_document lib must be written");
    std::fs::write(
        graph_document.join("model.rs"),
        "pub type JsonValue = serde_json::Value;\n",
    )
    .expect("fixture graph_document model must be written");
    std::fs::write(
        graph_document.join("constant_value.rs"),
        "use serde_json::Value;\npub fn decode(text: &str) -> Value { serde_json::from_str(text).unwrap() }\npub fn escape(value: &Value) { serde_json::to_string(value); }\n",
    ).expect("fixture constant literal owner must be written");
    let roots = vec![ProductionRoot {
        package_id: "graph-document-package".to_owned(),
        package: "yss-graph-document".to_owned(),
        target: "yss_graph_document".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: graph_document.join("lib.rs"),
    }];
    let modules = collect_production_modules(&fixture.root, &roots)
        .expect("fixture production modules must be discoverable");
    let classification =
        classify_rust_sources(&roots, &modules).expect("fixture production sources must classify");
    let raw = collect_production_dependencies(&fixture.root, &roots)
        .expect("fixture production dependencies must be discoverable");
    assert!(raw.iter().any(|dependency| {
        dependency.repository_relative_source_file
            == "src-tauri/crates/yss-graph-document/src/lib.rs"
            && dependency.written_target == "serde_json::Value"
    }));
    let dependencies = raw
        .iter()
        .filter(|dependency| dependency.written_target.starts_with("serde_json::"))
        .map(|dependency| CanonicalDependency {
            owning_package: dependency.owning_package.clone(),
            source_file: dependency.repository_relative_source_file.clone(),
            owner: dependency.fully_qualified_owner.clone(),
            kind: dependency.kind,
            mode: dependency.mode,
            origin: CanonicalOrigin::External(ExternalDependencyOrigin {
                declared_name: "serde_json".to_owned(),
                package_name: "serde_json".to_owned(),
                declaration_scope: CargoDependencyScope::Runtime,
                target_condition: None,
                canonical_subpath: dependency
                    .written_target
                    .strip_prefix("serde_json::")
                    .map(str::to_owned),
            }),
            canonical_origin_target: format!("external:{}", dependency.written_target),
            line: dependency.line,
            column: dependency.column,
        })
        .collect::<Vec<_>>();

    let violations =
        pure_leaf_graph_document_json_violations(&fixture.root, &dependencies, &classification);
    assert!(
        violations.iter().any(
            |violation| violation.rule_id == PURE_LEAF_GRAPH_DOCUMENT_JSON_RULE
                && violation.source_file == "src-tauri/crates/yss-graph-document/src/lib.rs"
        ),
        "production serde_json after a test module must be rejected: {violations:#?}"
    );
    let constant_violations = violations
        .iter()
        .filter(|violation| violation.source_file.ends_with("/constant_value.rs"))
        .collect::<Vec<_>>();
    assert_eq!(
        constant_violations.len(),
        1,
        "only serialization escapes the literal decoder capability: {constant_violations:#?}"
    );
}

#[test]
fn rust_project_production_does_not_depend_on_graph_layer() {
    let facts = production_facts();

    assert_eq!(
        project_to_graph_production_edges(&facts.dependencies, &facts.classification),
        Vec::<String>::new()
    );
}

#[test]
fn legacy_execution_runtime_and_project_store_mirrors_are_absent() {
    let root = repository_root();
    for relative in [
        "src-tauri/src/node_system",
        "src-tauri/src/execution/plan/legacy",
        "src-tauri/crates/yss-execution/src/plan/legacy",
    ] {
        assert!(
            !root.join(relative).exists(),
            "the removed legacy execution owner must not return: {relative}"
        );
    }

    let project_store =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-project/src/project_store.rs"))
            .expect("ProjectStore source must be readable");
    for removed_mirror in [
        "databases:",
        "node_registry:",
        "catalog:",
        "kernels:",
        "compiled_parameters:",
        "function_plans:",
        "results:",
        "memoization:",
        "runs:",
    ] {
        assert!(
            !project_store.contains(removed_mirror),
            "ProjectStore must not restore the test-only runtime mirror '{removed_mirror}'"
        );
    }
}

#[test]
fn graph_resource_contract_has_one_owner_distinct_from_builtin_catalog() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-graph-resource-contract/Cargo.toml",
        "src-tauri/crates/yss-graph-resource-contract/src/lib.rs",
        "src-tauri/crates/yss-graph-resource-contract/src/catalog.rs",
        "src-tauri/crates/yss-graph-resource-contract/src/schema.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "graph resource contract owner must exist at {relative}"
        );
    }
    for removed in [
        "src-tauri/src/graph/resource_catalog.rs",
        "src-tauri/src/graph/schema.rs",
    ] {
        assert!(
            !root.join(removed).exists(),
            "the root crate must not retain graph resource contract mirror {removed}"
        );
    }

    let source = std::fs::read_to_string(
        root.join("src-tauri/crates/yss-graph-resource-contract/src/lib.rs"),
    )
    .expect("graph resource contract root must be readable");
    assert!(
        source.contains("The built-in node catalog remains owned by `yss-graph-catalog`"),
        "resource snapshots and the built-in node catalog must remain distinct authorities"
    );
}

#[test]
fn tabular_value_boundary_retains_checked_construction_and_typed_errors() {
    let violations = tabular_contract_source_violations(&repository_root());
    assert!(
        violations.is_empty(),
        "{TABULAR_CONTRACT_RULE}: {violations:?}"
    );
}

#[test]
fn database_schema_and_arrow_adapter_have_separate_owners() {
    let facts = production_facts();
    for (source, layer) in [
        (
            "src-tauri/crates/yss-database-schema/src/lib.rs",
            RustLayer::PureLeaf,
        ),
        (
            "src-tauri/crates/yss-tabular-arrow/src/lib.rs",
            RustLayer::DatabaseCore,
        ),
        (
            "src-tauri/crates/yss-tabular-arrow/src/schema.rs",
            RustLayer::DatabaseCore,
        ),
    ] {
        assert_eq!(facts.classification.get(source), Some(&layer), "{source}");
    }
}

#[test]
fn diagnostics_has_one_crate_owner_separate_from_logging() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-diagnostics/Cargo.toml",
        "src-tauri/crates/yss-diagnostics/src/lib.rs",
        "src-tauri/crates/yss-diagnostics/src/dispatcher.rs",
        "src-tauri/crates/yss-diagnostics/src/dto.rs",
        "src-tauri/crates/yss-diagnostics/src/runtime.rs",
        "src-tauri/crates/yss-diagnostics/src/rust_projection.rs",
        "src-tauri/crates/yss-diagnostics/src/tests.rs",
        "src-tauri/crates/yss-diagnostics/src/validation.rs",
        "src-tauri/crates/yss-diagnostics/src/worker.rs",
        "src-tauri/crates/yss-tracing/Cargo.toml",
    ] {
        assert!(
            root.join(relative).is_file(),
            "diagnostics/logging owner must exist at {relative}"
        );
    }
    assert!(
        !root.join("src-tauri/src/diagnostics").exists(),
        "the root crate must not retain a diagnostics compatibility module"
    );
}

#[test]
fn chart_resource_cutover_does_not_keep_a_retired_rust_path() {
    fn inspect(path: &Path, root: &Path, retired: &str) {
        for entry in std::fs::read_dir(path).expect("Rust source directory must be readable") {
            let entry = entry.expect("Rust source entry must be readable");
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name == "target") {
                    continue;
                }
                inspect(&path, root, retired);
                continue;
            }

            let relative = path
                .strip_prefix(root)
                .expect("Rust source path must remain inside the repository")
                .to_string_lossy()
                .replace('\\', "/");
            if relative == "src-tauri/crates/yss-tabular-io/src/excel.rs" {
                continue;
            }
            assert!(
                !relative.to_lowercase().contains(retired),
                "retired chart resource path remains at {relative}"
            );

            let is_contract_source = matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("rs" | "toml")
            ) || path.file_name().is_some_and(|name| name == "Cargo.lock");
            if !is_contract_source {
                continue;
            }
            let source = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{relative} must be readable: {error}"));
            assert!(
                !source.to_lowercase().contains(retired),
                "retired chart resource symbol remains in {relative}"
            );
        }
    }

    let root = repository_root();
    let retired_resource_term = ["work", "sheet"].concat();
    inspect(&root.join("src-tauri"), &root, &retired_resource_term);
}

#[test]
fn project_watcher_has_one_lifecycle_owner_and_notify_adapter_crate() {
    let facts = production_facts();
    assert_eq!(
        facts
            .classification
            .get("src-tauri/crates/yss-project-watcher/src/lib.rs"),
        Some(&RustLayer::Application),
        "the platform-neutral watcher lifecycle must be an Application service"
    );
    assert_eq!(
        facts
            .classification
            .get("src-tauri/crates/yss-project-watcher-notify/src/lib.rs"),
        Some(&RustLayer::PlatformAdapter),
        "the Notify implementation must be a Platform adapter"
    );
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-project-watcher/Cargo.toml",
        "src-tauri/crates/yss-project-watcher/src/lib.rs",
        "src-tauri/crates/yss-project-watcher/src/tests.rs",
        "src-tauri/crates/yss-project-watcher-notify/Cargo.toml",
        "src-tauri/crates/yss-project-watcher-notify/src/lib.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "project watcher owner must exist at {relative}"
        );
    }
    assert!(
        !root
            .join("src-tauri/crates/yss-application/src/project_watcher.rs")
            .exists(),
        "the root Application must not retain the watcher lifecycle owner"
    );
    for removed_root_adapter in [
        "src-tauri/src/platform/mod.rs",
        "src-tauri/src/platform/project_file_watcher.rs",
    ] {
        assert!(
            !root.join(removed_root_adapter).exists(),
            "the root crate must not retain watcher adapter {removed_root_adapter}"
        );
    }
    for dependency in [
        "yss-project-change = { path = \"../yss-project-change\" }",
        "yss-project-filesystem = { path = \"../yss-project-filesystem\" }",
    ] {
        assert!(
            declares_dependency("yss-project-watcher", dependency),
            "project watcher must declare its canonical dependency {dependency}"
        );
    }
    for forbidden in ["notify", "tauri"] {
        assert!(
            !declares_dependency_family("yss-project-watcher", forbidden),
            "the platform-neutral watcher must not depend on {forbidden}"
        );
    }
    for dependency in [
        "notify.workspace = true",
        "yss-project-change = { path = \"../yss-project-change\" }",
        "yss-project-watcher = { path = \"../yss-project-watcher\" }",
    ] {
        assert!(
            declares_dependency("yss-project-watcher-notify", dependency),
            "Notify watcher must declare its canonical dependency {dependency}"
        );
    }
    assert!(
        !declares_dependency_family("yss-project-watcher-notify", "tauri"),
        "the Notify adapter must remain independent of Tauri"
    );
    let owner =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-project-watcher/src/lib.rs"))
            .expect("project watcher owner must be readable");
    for owned_api in [
        "pub struct ProjectWatcherState",
        "pub trait ProjectFileWatcherFactory",
        "pub trait ProjectChangeSink",
        "pub enum ProjectFileWatcherDrainOutcome",
    ] {
        assert!(
            owner.contains(owned_api),
            "project watcher crate must own {owned_api}"
        );
    }
    for removed_drift in ["PROJECT_WATCHER_QUIET_PERIOD", "DeliveryFailed"] {
        assert!(
            !owner.contains(removed_drift),
            "project watcher must not restore dead or duplicate fact {removed_drift}"
        );
    }
    let violations = project_watcher_source_violations(&repository_root());
    assert!(
        violations.is_empty(),
        "{PROJECT_WATCHER_BOUNDARY_RULE} violations: {violations:#?}"
    );
}

#[test]
fn categorical_role_owner_policy_requires_persisted_owner_and_only_approved_sci_origin() {
    let expectation = CanonicalOwnerExpectation {
        symbol: "CategoricalRole",
        required_origin: "src-tauri/crates/yss-data-contract/src/data_value.rs",
        allowed_origins: &[
            "src-tauri/crates/yss-data-contract/src/data_value.rs",
            "src-tauri/crates/yss-sci-contract/src/computation.rs",
        ],
    };

    assert!(canonical_owner_origins_are_valid(
        &expectation,
        &BTreeSet::from([
            "src-tauri/crates/yss-data-contract/src/data_value.rs",
            "src-tauri/crates/yss-sci-contract/src/computation.rs",
        ]),
    ));
    assert!(!canonical_owner_origins_are_valid(
        &expectation,
        &BTreeSet::from([
            "src-tauri/crates/yss-data-contract/src/data_value.rs",
            "src-tauri/src/sci/api/arbitrary.rs",
        ]),
    ));
    assert!(!canonical_owner_origins_are_valid(
        &expectation,
        &BTreeSet::from(["src-tauri/crates/yss-sci-contract/src/computation.rs"]),
    ));
}

#[test]
fn scientific_backend_contract_and_runtime_have_distinct_owners() {
    let facts = production_facts();
    assert_eq!(
        facts
            .classification
            .get("src-tauri/crates/yss-sci-contract/src/scientific.rs"),
        Some(&RustLayer::PureLeaf)
    );
    assert_eq!(
        facts
            .classification
            .get("src-tauri/crates/yss-sci-runtime/src/service.rs"),
        Some(&RustLayer::SciCore)
    );
    assert!(
        facts
            .dependencies
            .iter()
            .any(|dependency| dependency.source_file
                == "src-tauri/crates/yss-sci-runtime/src/service.rs"
                && dependency.canonical_origin_target
                    == "yss_sci_contract::scientific::ScientificBackend")
    );
    assert!(
        facts
            .dependencies
            .iter()
            .any(
                |dependency| dependency.source_file == "src-tauri/src/lib.rs"
                    && dependency.canonical_origin_target
                        == "yss_sci_runtime::service::SciRuntimeBackend::new"
            )
    );
    assert!(
        facts
            .dependencies
            .iter()
            .filter(|dependency| dependency
                .source_file
                .starts_with("src-tauri/crates/yss-execution/"))
            .all(|dependency| !dependency
                .canonical_origin_target
                .starts_with("yss_sci_runtime::")
                && !dependency.canonical_origin_target.starts_with("yss_sci::"))
    );
}

#[test]
fn persisted_contract_type_aliases_are_rejected_from_real_graph_and_sci_sources() {
    const FIXTURE_PREFIX: &str = "architecture-canonical-owner-alias-";

    struct SourceFixture {
        root: PathBuf,
    }

    impl SourceFixture {
        fn new() -> Self {
            let root = repository_root()
                .join("target")
                .join(format!("{FIXTURE_PREFIX}{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&root).expect("alias fixture root must be created");
            Self { root }
        }

        fn write(&self, relative: &str, source: &str) {
            let path = self.root.join(relative);
            assert!(path.starts_with(&self.root));
            std::fs::create_dir_all(path.parent().expect("fixture source must have a parent"))
                .expect("alias fixture source parent must be created");
            std::fs::write(path, source).expect("alias fixture source must be written");
        }
    }

    impl Drop for SourceFixture {
        fn drop(&mut self) {
            let safe_name = self
                .root
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(FIXTURE_PREFIX));
            if safe_name {
                let _ = std::fs::remove_dir_all(&self.root);
            }
        }
    }

    let fixture = SourceFixture::new();
    fixture.write("src-tauri/src/lib.rs", "pub mod graph;\n");
    fixture.write("src-tauri/src/graph/mod.rs", "pub mod value;\n");
    fixture.write("src-tauri/src/graph/value/mod.rs", "mod aliases;\n");
    fixture.write(
        "src-tauri/src/graph/value/aliases.rs",
        r#"
pub type PersistedDataType = yss_data_contract::DataType;
pub type PersistedDataValue = yss_data_contract::DataValue;
pub type PersistedDataSeriesValue = yss_data_contract::DataSeriesValue;
pub type PersistedCategoricalRole = yss_data_contract::CategoricalRole;
pub type PersistedTimeSeriesState = yss_data_contract::TimeSeriesState;
pub type PersistedDummyInfo = yss_data_contract::DummyInfo;

#[cfg(test)]
pub type TestOnlyAlias = yss_data_contract::DataType;
"#,
    );
    fixture.write(
        "src-tauri/crates/yss-sci-runtime/src/lib.rs",
        "pub mod api;\n",
    );
    fixture.write(
        "src-tauri/crates/yss-sci-runtime/src/api/mod.rs",
        "pub mod computation;\n",
    );
    fixture.write(
        "src-tauri/crates/yss-sci-runtime/src/api/computation.rs",
        r#"
pub enum CategoricalRole {
    Individual,
}

pub type PersistedCategoricalRole = yss_data_contract::CategoricalRole;
"#,
    );
    let modules = collect_production_modules(
        &fixture.root,
        &[
            ProductionRoot {
                package_id: "fixture-package".to_owned(),
                package: "fixture".to_owned(),
                target: "fixture_lib".to_owned(),
                kind: ProductionRootKind::Library,
                source_path: fixture.root.join("src-tauri/src/lib.rs"),
            },
            ProductionRoot {
                package_id: "fixture-sci-runtime-package".to_owned(),
                package: "yss-sci-runtime".to_owned(),
                target: "yss_sci_runtime".to_owned(),
                kind: ProductionRootKind::Library,
                source_path: fixture
                    .root
                    .join("src-tauri/crates/yss-sci-runtime/src/lib.rs"),
            },
        ],
    )
    .expect("real alias fixture modules must be discovered");
    let aliases = forbidden_persisted_contract_type_aliases(&fixture.root, &modules)
        .expect("real alias fixture must be scanned");
    let aliases = aliases
        .iter()
        .map(|alias| format!("{}|{}|{}", alias.source_file, alias.alias, alias.target))
        .collect::<Vec<_>>();
    assert_eq!(
        aliases,
        vec![
            "src-tauri/crates/yss-sci-runtime/src/api/computation.rs|PersistedCategoricalRole|yss_data_contract::CategoricalRole",
            "src-tauri/src/graph/value/aliases.rs|PersistedCategoricalRole|yss_data_contract::CategoricalRole",
            "src-tauri/src/graph/value/aliases.rs|PersistedDataSeriesValue|yss_data_contract::DataSeriesValue",
            "src-tauri/src/graph/value/aliases.rs|PersistedDataType|yss_data_contract::DataType",
            "src-tauri/src/graph/value/aliases.rs|PersistedDataValue|yss_data_contract::DataValue",
            "src-tauri/src/graph/value/aliases.rs|PersistedDummyInfo|yss_data_contract::DummyInfo",
            "src-tauri/src/graph/value/aliases.rs|PersistedTimeSeriesState|yss_data_contract::TimeSeriesState",
        ]
    );
}

#[test]
fn rust_build_script_and_external_dependency_policy_is_fail_closed() {
    const INTERNAL_CAPABILITIES: &[InternalDependencyCapability] =
        &[InternalDependencyCapability {
            source_layer: RustLayer::Commands,
            repository_relative_source_file: "src-tauri/crates/yss-api/src/commands/mod.rs",
            fully_qualified_owner: "fixture_lib::commands",
            canonical_origin_targets: &["fixture_lib::application::run"],
        }];
    const DECLARATIONS: &[ExternalDependencyDeclarationAllowance] = &[
        ExternalDependencyDeclarationAllowance {
            owning_package: "fixture",
            mode: RustDependencyMode::Build,
            package_name: "tauri-build",
            target_condition: None,
        },
        ExternalDependencyDeclarationAllowance {
            owning_package: "fixture",
            mode: RustDependencyMode::Runtime,
            package_name: "tauri",
            target_condition: None,
        },
    ];
    const USES: &[ExternalDependencyUseAllowance] = &[
        ExternalDependencyUseAllowance {
            source_layer: RustLayer::BuildScript,
            mode: RustDependencyMode::Build,
            package_name: "tauri-build",
        },
        ExternalDependencyUseAllowance {
            source_layer: RustLayer::Commands,
            mode: RustDependencyMode::Runtime,
            package_name: "tauri",
        },
    ];
    const POLICY: ExternalDependencyPolicy = ExternalDependencyPolicy {
        declarations: DECLARATIONS,
        uses: USES,
    };

    let declarations = vec![
        external_declaration("tauri-build", CargoDependencyScope::Build),
        external_declaration("tauri", CargoDependencyScope::Runtime),
        external_declaration("proc-macro2", CargoDependencyScope::Development),
    ];
    let classification = BTreeMap::from([
        ("src-tauri/build.rs".to_owned(), RustLayer::BuildScript),
        (
            "src-tauri/crates/yss-api/src/commands/mod.rs".to_owned(),
            RustLayer::Commands,
        ),
        ("src-tauri/src/graph/mod.rs".to_owned(), RustLayer::Graph),
        (
            "src-tauri/crates/yss-application/src/lib.rs".to_owned(),
            RustLayer::Application,
        ),
        (
            "src-tauri/crates/yss-sci/src/api/computation.rs".to_owned(),
            RustLayer::SciCore,
        ),
    ]);

    let approved_build = external_dependency(
        "src-tauri/build.rs",
        "build_script_build",
        RustDependencyMode::Build,
        "tauri-build",
        CargoDependencyScope::Build,
        "build",
    );
    let approved_command = external_dependency(
        "src-tauri/crates/yss-api/src/commands/mod.rs",
        "fixture_lib::commands",
        RustDependencyMode::Runtime,
        "tauri",
        CargoDependencyScope::Runtime,
        "Builder",
    );
    assert!(
        rust_external_dependency_findings(
            &declarations,
            &[approved_build, approved_command],
            &classification,
            &POLICY,
        )
        .expect("approved external rows must audit")
        .is_empty()
    );

    let forbidden_external = [
        external_dependency(
            "src-tauri/build.rs",
            "build_script_build",
            RustDependencyMode::Runtime,
            "tauri",
            CargoDependencyScope::Runtime,
            "Builder",
        ),
        external_dependency(
            "src-tauri/src/graph/mod.rs",
            "fixture_lib::graph",
            RustDependencyMode::Runtime,
            "tauri",
            CargoDependencyScope::Runtime,
            "Builder",
        ),
    ];
    let findings = rust_external_dependency_findings(
        &declarations,
        &forbidden_external,
        &classification,
        &POLICY,
    )
    .expect("known packages used from a forbidden layer must become findings");
    assert_eq!(findings.len(), 2);
    assert!(
        findings
            .iter()
            .all(|finding| finding.key.rule_id == "rust.external.runtime-source-layer")
    );

    let unknown = external_dependency(
        "src-tauri/src/graph/mod.rs",
        "fixture_lib::graph",
        RustDependencyMode::Runtime,
        "mystery-crate",
        CargoDependencyScope::Runtime,
        "Thing",
    );
    assert!(matches!(
        rust_external_dependency_findings(
            &declarations,
            &[unknown],
            &classification,
            &POLICY,
        ),
        Err(ArchitectureAuditError::UnknownExternalPackage { package_name })
            if package_name == "mystery-crate"
    ));

    let development_only = external_dependency(
        "src-tauri/src/graph/mod.rs",
        "fixture_lib::graph",
        RustDependencyMode::Runtime,
        "proc-macro2",
        CargoDependencyScope::Development,
        "TokenStream",
    );
    assert!(matches!(
        rust_external_dependency_findings(
            &declarations,
            &[development_only],
            &classification,
            &POLICY,
        ),
        Err(ArchitectureAuditError::DevelopmentDependencyInProduction { target })
            if target == "proc-macro2"
    ));

    let workspace_member = CanonicalDependency {
        owning_package: "fixture".to_owned(),
        source_file: "src-tauri/crates/yss-application/src/lib.rs".to_owned(),
        owner: "fixture_lib::application".to_owned(),
        kind: RustDependencyKind::Use,
        mode: RustDependencyMode::Runtime,
        origin: CanonicalOrigin::Repository {
            package_name: "yss-sci".to_owned(),
            repository_relative_declaration_file: "src-tauri/crates/yss-sci/src/api/computation.rs"
                .to_owned(),
            fully_qualified_target: "yss_sci::api::computation::StatisticalInput".to_owned(),
            symbol: "StatisticalInput".to_owned(),
        },
        canonical_origin_target: "yss_sci::api::computation::StatisticalInput".to_owned(),
        line: 1,
        column: 1,
    };
    assert!(
        rust_dependency_findings(&[workspace_member], &classification)
            .expect("the static internal dependency policy must audit")
            .is_empty()
    );

    let approved_command_seam = CanonicalDependency {
        owning_package: "fixture".to_owned(),
        source_file: "src-tauri/crates/yss-api/src/commands/mod.rs".to_owned(),
        owner: "fixture_lib::commands".to_owned(),
        kind: RustDependencyKind::Path,
        mode: RustDependencyMode::Runtime,
        origin: CanonicalOrigin::Repository {
            package_name: "fixture".to_owned(),
            repository_relative_declaration_file: "src-tauri/crates/yss-application/src/lib.rs"
                .to_owned(),
            fully_qualified_target: "fixture_lib::application::run".to_owned(),
            symbol: "run".to_owned(),
        },
        canonical_origin_target: "fixture_lib::application::run".to_owned(),
        line: 1,
        column: 1,
    };
    assert!(
        rust_dependency_findings_with_capabilities(
            &[approved_command_seam.clone()],
            &classification,
            INTERNAL_CAPABILITIES,
        )
        .expect("an exact internal capability manifest must audit")
        .is_empty()
    );

    let mut moved_command_seam = approved_command_seam;
    moved_command_seam.source_file = "src-tauri/src/graph/mod.rs".to_owned();
    moved_command_seam.owner = "fixture_lib::graph".to_owned();
    assert_eq!(
        rust_dependency_findings_with_capabilities(
            &[moved_command_seam],
            &classification,
            INTERNAL_CAPABILITIES,
        )
        .expect("a moved caller must become an ordinary finding")
        .len(),
        1
    );

    const INVALID_INTERNAL_CAPABILITIES: &[InternalDependencyCapability] =
        &[InternalDependencyCapability {
            source_layer: RustLayer::Commands,
            repository_relative_source_file: "src-tauri/crates/yss-api/src/commands/*",
            fully_qualified_owner: "fixture_lib::commands",
            canonical_origin_targets: &["fixture_lib::application::*"],
        }];
    assert!(matches!(
        rust_dependency_findings_with_capabilities(
            &[],
            &classification,
            INVALID_INTERNAL_CAPABILITIES,
        ),
        Err(ArchitectureAuditError::InvalidInternalCapability { .. })
    ));

    let build_to_application = CanonicalDependency {
        owning_package: "fixture".to_owned(),
        source_file: "src-tauri/build.rs".to_owned(),
        owner: "build_script_build".to_owned(),
        kind: RustDependencyKind::Path,
        mode: RustDependencyMode::Build,
        origin: CanonicalOrigin::Repository {
            package_name: "fixture".to_owned(),
            repository_relative_declaration_file: "src-tauri/crates/yss-application/src/lib.rs"
                .to_owned(),
            fully_qualified_target: "fixture_lib::application::ApplicationState".to_owned(),
            symbol: "ApplicationState".to_owned(),
        },
        canonical_origin_target: "fixture_lib::application::ApplicationState".to_owned(),
        line: 1,
        column: 1,
    };
    assert_eq!(
        rust_dependency_findings(&[build_to_application], &classification)
            .expect("the static internal dependency policy must audit")
            .len(),
        1
    );
}

#[test]
fn rust_production_architecture_matches_declared_policy() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let workspace = super::cargo_targets::discover_rust_workspace_model(&manifest)
        .expect("the real Cargo workspace must be discoverable");
    let modules = collect_production_modules(&workspace.repository_root, &workspace.roots)
        .expect("the production module graph must be discoverable");
    let raw_dependencies =
        collect_production_dependencies(&workspace.repository_root, &workspace.roots)
            .expect("production dependency facts must be discoverable");
    let dependencies = resolve_canonical_dependencies_detailed(&workspace, &raw_dependencies)
        .unwrap_or_else(|failure| {
            panic!("every production dependency must resolve to a canonical origin: {failure:#?}")
        });
    let classification = classify_rust_sources(&workspace.roots, &modules)
        .expect("every production source must classify exactly once");

    let mut findings = rust_dependency_findings(&dependencies, &classification)
        .expect("internal dependency capabilities must be auditable");
    findings.extend(
        rust_external_dependency_findings(
            &workspace.dependency_declarations,
            &dependencies,
            &classification,
            &RUST_EXTERNAL_DEPENDENCY_POLICY,
        )
        .expect("external declarations and uses must be auditable"),
    );
    findings.sort();

    assert!(
        findings.is_empty(),
        "real Rust production architecture violates the declared policy:\n{findings:#?}"
    );
}

#[test]
fn sample_catalog_composition_capability_does_not_grant_business_queries() {
    let source = "src-tauri/src/lib.rs";
    let declaration = "src-tauri/crates/yss-application/src/database/samples.rs";
    let target = "yss_application::database::samples::SampleCatalog::new";
    let classification = BTreeMap::from([
        (source.to_owned(), RustLayer::CompositionRoot),
        (declaration.to_owned(), RustLayer::Application),
    ]);
    let mut dependency = CanonicalDependency {
        owning_package: "yssbi".to_owned(),
        source_file: source.to_owned(),
        owner: "yssbi_lib".to_owned(),
        kind: RustDependencyKind::Path,
        mode: RustDependencyMode::Runtime,
        origin: CanonicalOrigin::Repository {
            package_name: "yss-application".to_owned(),
            repository_relative_declaration_file: declaration.to_owned(),
            fully_qualified_target: target.to_owned(),
            symbol: "new".to_owned(),
        },
        canonical_origin_target: target.to_owned(),
        line: 1,
        column: 1,
    };
    assert!(
        rust_dependency_findings(&[dependency.clone()], &classification)
            .unwrap()
            .is_empty()
    );
    let query = "yss_application::database::samples::SampleCatalog::list";
    dependency.canonical_origin_target = query.to_owned();
    if let CanonicalOrigin::Repository {
        fully_qualified_target,
        symbol,
        ..
    } = &mut dependency.origin
    {
        *fully_qualified_target = query.to_owned();
        *symbol = "list".to_owned();
    }
    assert_eq!(
        rust_dependency_findings(&[dependency], &classification)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn result_projection_capability_does_not_grant_application_state_access() {
    let source = "src-tauri/crates/yss-api/src/schema/result.rs";
    let projection = "src-tauri/crates/yss-application/src/execution/result_query/report.rs";
    let state = "src-tauri/crates/yss-application/src/execution/session_slot.rs";
    let classification = BTreeMap::from([
        (source.to_owned(), RustLayer::Transport),
        (projection.to_owned(), RustLayer::Application),
        (state.to_owned(), RustLayer::Application),
    ]);
    let dependency = |declaration: &str, target: &str, symbol: &str| CanonicalDependency {
        owning_package: "yss-api".into(),
        source_file: source.into(),
        owner: "yss_api::schema::result".into(),
        kind: RustDependencyKind::Use,
        mode: RustDependencyMode::Runtime,
        origin: CanonicalOrigin::Repository {
            package_name: "yss-application".into(),
            repository_relative_declaration_file: declaration.into(),
            fully_qualified_target: target.into(),
            symbol: symbol.into(),
        },
        canonical_origin_target: target.into(),
        line: 1,
        column: 1,
    };
    assert!(
        rust_dependency_findings(
            &[dependency(
                projection,
                "yss_application::execution::result_query::report::OlsReportProjection",
                "OlsReportProjection"
            )],
            &classification
        )
        .unwrap()
        .is_empty()
    );
    assert_eq!(
        rust_dependency_findings(
            &[dependency(
                state,
                "yss_application::execution::session_slot::ApplicationState",
                "ApplicationState"
            )],
            &classification
        )
        .unwrap()
        .len(),
        1
    );
}

fn external_declaration(
    package_name: &str,
    scope: CargoDependencyScope,
) -> CargoDependencyDeclaration {
    CargoDependencyDeclaration {
        owning_package_id: "fixture-package".to_owned(),
        owning_package: "fixture".to_owned(),
        declared_name: package_name.replace('-', "_"),
        package_name: package_name.to_owned(),
        authority: CargoDependencyAuthority::External,
        scope,
        target_condition: None,
    }
}

fn external_dependency(
    source_file: &str,
    owner: &str,
    mode: RustDependencyMode,
    package_name: &str,
    declaration_scope: CargoDependencyScope,
    subpath: &str,
) -> CanonicalDependency {
    CanonicalDependency {
        owning_package: "fixture".to_owned(),
        source_file: source_file.to_owned(),
        owner: owner.to_owned(),
        kind: RustDependencyKind::Path,
        mode,
        origin: CanonicalOrigin::External(ExternalDependencyOrigin {
            declared_name: package_name.replace('-', "_"),
            package_name: package_name.to_owned(),
            declaration_scope,
            target_condition: None,
            canonical_subpath: Some(subpath.to_owned()),
        }),
        canonical_origin_target: format!("external:{package_name}::{subpath}"),
        line: 1,
        column: 1,
    }
}
