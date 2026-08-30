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
    graph_project_revision_bridge_violations, project_to_graph_production_edges,
    project_watcher_source_violations, pure_leaf_graph_document_json_violations,
    tabular_contract_source_violations,
};

fn repository_root() -> PathBuf {
    let manifest_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri must have a repository parent")
        .to_path_buf();
    std::fs::canonicalize(manifest_root).expect("repository root must be canonicalizable")
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
            .any(|root| root.package == "yss-graph-document"
                && root.target == "yss_graph_document"
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
    assert!(workspace.roots.iter().any(|root| root.package == "yss-math"
        && root.target == "yss_math"
        && root.kind == ProductionRootKind::Library));
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
            .any(|root| root.package == "yss-variable-contract"
                && root.target == "yss_variable_contract"
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
                alias.owning_package == "yssbi"
                    && alias.declared_name == "yss_sci"
                    && alias.member_package == "yss-sci"
            })
    );
    assert!(workspace.dependency_declarations.iter().any(|dependency| {
        dependency.owning_package == "yssbi"
            && dependency.package_name == "yss-sci"
            && matches!(
                dependency.authority,
                CargoDependencyAuthority::WorkspaceMember { .. }
            )
    }));
    assert!(
        workspace
            .workspace_member_crate_aliases
            .iter()
            .any(|alias| {
                alias.owning_package == "yssbi"
                    && alias.declared_name == "yss_data_contract"
                    && alias.member_package == "yss-data-contract"
            })
    );
    assert!(workspace.dependency_declarations.iter().any(|dependency| {
        dependency.owning_package == "yssbi"
            && dependency.package_name == "yss-data-contract"
            && matches!(
                dependency.authority,
                CargoDependencyAuthority::WorkspaceMember { .. }
            )
    }));
    assert!(
        workspace
            .workspace_member_crate_aliases
            .iter()
            .any(|alias| {
                alias.owning_package == "yssbi"
                    && alias.declared_name == "yss_database_contract"
                    && alias.member_package == "yss-database-contract"
            })
    );
    assert!(workspace.dependency_declarations.iter().any(|dependency| {
        dependency.owning_package == "yssbi"
            && dependency.package_name == "yss-database-contract"
            && matches!(
                dependency.authority,
                CargoDependencyAuthority::WorkspaceMember { .. }
            )
    }));
    assert!(
        workspace
            .workspace_member_crate_aliases
            .iter()
            .any(|alias| {
                alias.owning_package == "yssbi"
                    && alias.declared_name == "yss_diagnostics"
                    && alias.member_package == "yss-diagnostics"
            })
    );
    assert!(workspace.dependency_declarations.iter().any(|dependency| {
        dependency.owning_package == "yssbi"
            && dependency.package_name == "yss-diagnostics"
            && matches!(
                dependency.authority,
                CargoDependencyAuthority::WorkspaceMember { .. }
            )
    }));
    assert!(
        workspace
            .workspace_member_crate_aliases
            .iter()
            .any(|alias| {
                alias.owning_package == "yssbi"
                    && alias.declared_name == "yss_graph_document"
                    && alias.member_package == "yss-graph-document"
            })
    );
    assert!(workspace.dependency_declarations.iter().any(|dependency| {
        dependency.owning_package == "yssbi"
            && dependency.package_name == "yss-graph-document"
            && matches!(
                dependency.authority,
                CargoDependencyAuthority::WorkspaceMember { .. }
            )
    }));
    assert!(
        workspace
            .workspace_member_crate_aliases
            .iter()
            .any(|alias| {
                alias.owning_package == "yssbi"
                    && alias.declared_name == "yss_graph_protocol"
                    && alias.member_package == "yss-graph-protocol"
            })
    );
    assert!(workspace.dependency_declarations.iter().any(|dependency| {
        dependency.owning_package == "yssbi"
            && dependency.package_name == "yss-graph-protocol"
            && matches!(
                dependency.authority,
                CargoDependencyAuthority::WorkspaceMember { .. }
            )
    }));
    assert!(
        workspace
            .workspace_member_crate_aliases
            .iter()
            .any(|alias| {
                alias.owning_package == "yssbi"
                    && alias.declared_name == "yss_math"
                    && alias.member_package == "yss-math"
            })
    );
    assert!(workspace.dependency_declarations.iter().any(|dependency| {
        dependency.owning_package == "yssbi"
            && dependency.package_name == "yss-math"
            && matches!(
                dependency.authority,
                CargoDependencyAuthority::WorkspaceMember { .. }
            )
    }));
    assert!(
        workspace
            .workspace_member_crate_aliases
            .iter()
            .any(|alias| {
                alias.owning_package == "yssbi"
                    && alias.declared_name == "yss_tabular_contract"
                    && alias.member_package == "yss-tabular-contract"
            })
    );
    assert!(workspace.dependency_declarations.iter().any(|dependency| {
        dependency.owning_package == "yssbi"
            && dependency.package_name == "yss-tabular-contract"
            && matches!(
                dependency.authority,
                CargoDependencyAuthority::WorkspaceMember { .. }
            )
    }));
    assert!(
        workspace
            .workspace_member_crate_aliases
            .iter()
            .any(|alias| {
                alias.owning_package == "yssbi"
                    && alias.declared_name == "yss_variable_contract"
                    && alias.member_package == "yss-variable-contract"
            })
    );
    assert!(workspace.dependency_declarations.iter().any(|dependency| {
        dependency.owning_package == "yssbi"
            && dependency.package_name == "yss-variable-contract"
            && matches!(
                dependency.authority,
                CargoDependencyAuthority::WorkspaceMember { .. }
            )
    }));
    assert!(
        workspace
            .workspace_member_crate_aliases
            .iter()
            .any(|alias| {
                alias.owning_package == "yssbi"
                    && alias.declared_name == "yss_window_state"
                    && alias.member_package == "yss-window-state"
            })
    );
    assert!(workspace.dependency_declarations.iter().any(|dependency| {
        dependency.owning_package == "yssbi"
            && dependency.package_name == "yss-window-state"
            && matches!(
                dependency.authority,
                CargoDependencyAuthority::WorkspaceMember { .. }
            )
    }));
    assert!(
        workspace
            .workspace_member_crate_aliases
            .iter()
            .any(|alias| {
                alias.owning_package == "yssbi"
                    && alias.declared_name == "yss_tracing"
                    && alias.member_package == "yss-tracing"
            })
    );
    assert!(workspace.dependency_declarations.iter().any(|dependency| {
        dependency.owning_package == "yssbi"
            && dependency.package_name == "yss-tracing"
            && matches!(
                dependency.authority,
                CargoDependencyAuthority::WorkspaceMember { .. }
            )
    }));
    assert!(
        workspace
            .roots
            .iter()
            .all(|root| root.source_path.starts_with(&workspace.repository_root))
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
    let data_contract_root = ProductionRoot {
        package_id: "data-contract-package".to_owned(),
        package: "yss-data-contract".to_owned(),
        target: "yss_data_contract".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-data-contract/src/lib.rs"),
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
    let graph_document_root = ProductionRoot {
        package_id: "graph-document-package".to_owned(),
        package: "yss-graph-document".to_owned(),
        target: "yss_graph_document".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-graph-document/src/lib.rs"),
    };
    let graph_protocol_root = ProductionRoot {
        package_id: "graph-protocol-package".to_owned(),
        package: "yss-graph-protocol".to_owned(),
        target: "yss_graph_protocol".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-graph-protocol/src/lib.rs"),
    };
    let math_root = ProductionRoot {
        package_id: "math-package".to_owned(),
        package: "yss-math".to_owned(),
        target: "yss_math".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-math/src/lib.rs"),
    };
    let tabular_contract_root = ProductionRoot {
        package_id: "tabular-contract-package".to_owned(),
        package: "yss-tabular-contract".to_owned(),
        target: "yss_tabular_contract".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-tabular-contract/src/lib.rs"),
    };
    let variable_contract_root = ProductionRoot {
        package_id: "variable-contract-package".to_owned(),
        package: "yss-variable-contract".to_owned(),
        target: "yss_variable_contract".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-variable-contract/src/lib.rs"),
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
        data_contract_root.clone(),
        database_contract_root.clone(),
        diagnostics_root.clone(),
        graph_document_root.clone(),
        graph_protocol_root.clone(),
        math_root.clone(),
        tabular_contract_root.clone(),
        variable_contract_root.clone(),
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
                &runtime_root,
                "src-tauri/src/application/mod.rs",
                "fixture_lib::application",
            ),
            module(
                &runtime_root,
                "src-tauri/src/graph/catalog/builtin.rs",
                "fixture_lib::graph::catalog::builtin",
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
                &graph_document_root,
                "src-tauri/crates/yss-graph-document/src/lib.rs",
                "yss_graph_document",
            ),
            module(
                &graph_protocol_root,
                "src-tauri/crates/yss-graph-protocol/src/lib.rs",
                "yss_graph_protocol",
            ),
            module(
                &math_root,
                "src-tauri/crates/yss-math/src/lib.rs",
                "yss_math",
            ),
            module(
                &tabular_contract_root,
                "src-tauri/crates/yss-tabular-contract/src/lib.rs",
                "yss_tabular_contract",
            ),
            module(
                &variable_contract_root,
                "src-tauri/crates/yss-variable-contract/src/lib.rs",
                "yss_variable_contract",
            ),
            module(
                &window_state_root,
                "src-tauri/crates/yss-window-state/src/lib.rs",
                "yss_window_state",
            ),
            module(
                &runtime_root,
                "src-tauri/src/execution/settings.rs",
                "fixture_lib::execution::settings",
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
        classified["src-tauri/src/application/mod.rs"],
        RustLayer::Application
    );
    assert_eq!(
        classified["src-tauri/src/graph/catalog/builtin.rs"],
        RustLayer::BuiltinComposition
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
        classified["src-tauri/crates/yss-graph-document/src/lib.rs"],
        RustLayer::PureLeaf
    );
    assert_eq!(
        classified["src-tauri/crates/yss-graph-protocol/src/lib.rs"],
        RustLayer::PureLeaf
    );
    assert_eq!(
        classified["src-tauri/crates/yss-math/src/lib.rs"],
        RustLayer::PureLeaf
    );
    assert_eq!(
        classified["src-tauri/crates/yss-tabular-contract/src/lib.rs"],
        RustLayer::PureLeaf
    );
    assert_eq!(
        classified["src-tauri/crates/yss-variable-contract/src/lib.rs"],
        RustLayer::PureLeaf
    );
    assert_eq!(
        classified["src-tauri/crates/yss-window-state/src/lib.rs"],
        RustLayer::PlatformAdapter
    );
    assert_eq!(
        classified["src-tauri/src/execution/settings.rs"],
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
            "src-tauri/src/application/mod.rs",
            "fixture_lib::application",
        )],
    )
    .expect_err("BuildScript membership must not hide a second layer match");
    assert!(matches!(
        overlap,
        ArchitectureAuditError::MultiplyClassifiedProductionSource { source_files }
            if source_files == vec!["src-tauri/src/application/mod.rs"]
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
    } else if source_file.starts_with("src-tauri/src/sci/") {
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
fn persisted_data_contract_has_one_pure_owner_without_graph_compatibility_reexport() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let workspace = super::cargo_targets::discover_rust_workspace_model(&manifest)
        .expect("the real Cargo workspace must be discoverable");
    let modules = collect_production_modules(&workspace.repository_root, &workspace.roots)
        .expect("the production module graph must be discoverable");
    let forbidden_contract_aliases =
        forbidden_persisted_contract_type_aliases(&workspace.repository_root, &modules)
            .expect("production persisted contract aliases must be discoverable");
    assert!(
        forbidden_contract_aliases.is_empty(),
        "Graph must not alias persisted data-contract symbols, and SCI must not alias persisted CategoricalRole: {forbidden_contract_aliases:#?}"
    );
    let classification = classify_rust_sources(&workspace.roots, &modules)
        .expect("every production source must classify exactly once");

    for source in [
        "src-tauri/crates/yss-data-contract/src/lib.rs",
        "src-tauri/crates/yss-data-contract/src/data_type.rs",
        "src-tauri/crates/yss-data-contract/src/data_value.rs",
        "src-tauri/crates/yss-graph-document/src/lib.rs",
        "src-tauri/crates/yss-graph-document/src/identity.rs",
        "src-tauri/crates/yss-graph-document/src/model.rs",
        "src-tauri/crates/yss-graph-document/src/name.rs",
        "src-tauri/crates/yss-graph-document/src/resource_path.rs",
    ] {
        assert_eq!(classification.get(source), Some(&RustLayer::PureLeaf));
    }
    assert!(
        !workspace
            .repository_root
            .join("src-tauri/src/graph/value")
            .exists(),
        "the root crate must not retain an unused graph value compatibility module"
    );

    let raw_dependencies =
        collect_production_dependencies(&workspace.repository_root, &workspace.roots)
            .expect("production dependency facts must be discoverable");
    let dependencies = resolve_canonical_dependencies_detailed(&workspace, &raw_dependencies)
        .unwrap_or_else(|failure| {
            panic!("every production dependency must resolve to a canonical origin: {failure:#?}")
        });

    let forbidden_contract_reexports = dependencies
        .iter()
        .filter(|dependency| {
            if dependency.kind != RustDependencyKind::ReExport {
                return false;
            }

            matches!(
                &dependency.origin,
                CanonicalOrigin::Repository {
                    repository_relative_declaration_file,
                    symbol,
                    ..
                } if repository_relative_declaration_file.starts_with("src-tauri/crates/yss-data-contract/src/")
                    && (dependency.source_file.starts_with("src-tauri/src/graph/")
                        || (dependency.source_file.starts_with("src-tauri/src/sci/")
                            && symbol == "CategoricalRole"))
            )
        })
        .collect::<Vec<_>>();
    assert!(
        forbidden_contract_reexports.is_empty(),
        "Graph must not re-export the persisted data contract, and SCI must not re-export or alias its CategoricalRole: {forbidden_contract_reexports:#?}"
    );

    for expectation in [
        CanonicalOwnerExpectation {
            symbol: "DataType",
            required_origin: "src-tauri/crates/yss-data-contract/src/data_type.rs",
            allowed_origins: &["src-tauri/crates/yss-data-contract/src/data_type.rs"],
        },
        CanonicalOwnerExpectation {
            symbol: "DataValue",
            required_origin: "src-tauri/crates/yss-data-contract/src/data_value.rs",
            allowed_origins: &["src-tauri/crates/yss-data-contract/src/data_value.rs"],
        },
        CanonicalOwnerExpectation {
            symbol: "DataSeriesValue",
            required_origin: "src-tauri/crates/yss-data-contract/src/data_value.rs",
            allowed_origins: &["src-tauri/crates/yss-data-contract/src/data_value.rs"],
        },
        CanonicalOwnerExpectation {
            symbol: "CategoricalRole",
            required_origin: "src-tauri/crates/yss-data-contract/src/data_value.rs",
            allowed_origins: &[
                "src-tauri/crates/yss-data-contract/src/data_value.rs",
                "src-tauri/src/sci/api/computation.rs",
            ],
        },
        CanonicalOwnerExpectation {
            symbol: "TimeSeriesState",
            required_origin: "src-tauri/crates/yss-data-contract/src/data_value.rs",
            allowed_origins: &["src-tauri/crates/yss-data-contract/src/data_value.rs"],
        },
        CanonicalOwnerExpectation {
            symbol: "DummyInfo",
            required_origin: "src-tauri/crates/yss-data-contract/src/data_value.rs",
            allowed_origins: &["src-tauri/crates/yss-data-contract/src/data_value.rs"],
        },
    ] {
        let origins = dependencies
            .iter()
            .filter_map(|dependency| match &dependency.origin {
                CanonicalOrigin::Repository {
                    repository_relative_declaration_file,
                    symbol: origin_symbol,
                    ..
                } if origin_symbol == expectation.symbol => {
                    Some(repository_relative_declaration_file.as_str())
                }
                CanonicalOrigin::Repository { .. }
                | CanonicalOrigin::LanguageBuiltin { .. }
                | CanonicalOrigin::RepositoryAsset { .. }
                | CanonicalOrigin::External(_) => None,
            })
            .collect::<BTreeSet<_>>();
        assert!(
            canonical_owner_origins_are_valid(&expectation, &origins),
            "{} must retain {} and resolve only to approved owners {:?}, got {:?}",
            expectation.symbol,
            expectation.required_origin,
            expectation.allowed_origins,
            origins,
        );
    }
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
fn rust_pure_leaf_json_guard_rejects_production_use_after_test_module() {
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
        "mod model;\n#[cfg(test)] mod tests {}\npub type RuntimeEscape = serde_json::Value;\n",
    )
    .expect("fixture graph_document lib must be written");
    std::fs::write(
        graph_document.join("model.rs"),
        "pub type TypedValue = serde_json::Value;\n",
    )
    .expect("fixture graph_document model must be written");
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
        .filter(|dependency| dependency.written_target == "serde_json::Value")
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
                canonical_subpath: Some("Value".to_owned()),
            }),
            canonical_origin_target: "external:serde_json::Value".to_owned(),
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
fn rust_graph_project_revision_conversions_are_explicit() {
    assert_eq!(
        graph_project_revision_bridge_violations(&repository_root()),
        Vec::<String>::new()
    );
}

#[test]
fn database_contract_has_one_pure_crate_owner_without_compatibility_module() {
    let root = repository_root();
    let contract_files = [
        "src-tauri/crates/yss-database-contract/src/lib.rs",
        "src-tauri/crates/yss-database-contract/src/declaration.rs",
        "src-tauri/crates/yss-database-contract/src/engine.rs",
        "src-tauri/crates/yss-database-contract/src/fingerprint.rs",
        "src-tauri/crates/yss-database-contract/src/identity.rs",
        "src-tauri/crates/yss-database-contract/src/observation.rs",
        "src-tauri/crates/yss-database-contract/src/session.rs",
    ];
    for relative in contract_files {
        assert!(
            root.join(relative).is_file(),
            "database contract owner must exist at {relative}"
        );
    }

    for relative in [
        "src-tauri/src/database_contract",
        "src-tauri/src/database/database_decl.rs",
        "src-tauri/src/database/database_engine.rs",
        "src-tauri/src/database/database_engine_sql.rs",
    ] {
        assert!(
            !root.join(relative).exists(),
            "legacy database declaration owner must be removed: {relative}"
        );
    }
}

#[test]
fn tabular_contract_and_adapters_are_acyclic_and_typed() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-tabular-contract/Cargo.toml",
        "src-tauri/crates/yss-tabular-contract/src/lib.rs",
        "src-tauri/crates/yss-tabular-contract/tests/wire_contract.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "tabular contract owner must exist at {relative}"
        );
    }
    assert!(
        !root.join("src-tauri/src/tabular").exists(),
        "the root crate must not retain a tabular compatibility module"
    );
    let violations = tabular_contract_source_violations(&root);
    assert!(
        violations.is_empty(),
        "{TABULAR_CONTRACT_RULE} violations: {violations:#?}"
    );
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
fn graph_document_has_one_pure_crate_owner_without_compatibility_module() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-graph-document/Cargo.toml",
        "src-tauri/crates/yss-graph-document/src/identity.rs",
        "src-tauri/crates/yss-graph-document/src/lib.rs",
        "src-tauri/crates/yss-graph-document/src/model.rs",
        "src-tauri/crates/yss-graph-document/src/name.rs",
        "src-tauri/crates/yss-graph-document/src/resource_path.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "graph document owner must exist at {relative}"
        );
    }
    assert!(
        !root.join("src-tauri/src/graph_document").exists(),
        "the root crate must not retain a graph document compatibility module"
    );

    let project_name_source =
        std::fs::read_to_string(root.join("src-tauri/src/project/resource_name.rs"))
            .expect("the project resource-name allocator must be readable");
    for duplicate in [
        "enum ResourceNameError",
        "const MAX_RESOURCE_NAME_CHARACTERS",
    ] {
        assert!(
            !project_name_source.contains(duplicate),
            "project must consume the canonical graph-document name rule instead of duplicating {duplicate}"
        );
    }
}

#[test]
fn graph_protocol_has_one_pure_crate_owner_without_compatibility_module() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-graph-protocol/Cargo.toml",
        "src-tauri/crates/yss-graph-protocol/src/data_series.rs",
        "src-tauri/crates/yss-graph-protocol/src/dataframe.rs",
        "src-tauri/crates/yss-graph-protocol/src/identity.rs",
        "src-tauri/crates/yss-graph-protocol/src/lib.rs",
        "src-tauri/crates/yss-graph-protocol/src/model.rs",
        "src-tauri/crates/yss-graph-protocol/src/parameter.rs",
        "src-tauri/crates/yss-graph-protocol/src/tests.rs",
        "src-tauri/crates/yss-graph-protocol/src/types.rs",
        "src-tauri/crates/yss-graph-protocol/src/validation.rs",
        "src-tauri/crates/yss-graph-protocol/src/value.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "graph protocol owner must exist at {relative}"
        );
    }
    assert!(
        !root.join("src-tauri/src/graph/protocol").exists(),
        "the root crate must not retain a graph protocol compatibility module"
    );
}

#[test]
fn math_parser_has_one_pure_crate_owner_without_compatibility_module() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-math/Cargo.toml",
        "src-tauri/crates/yss-math/src/lib.rs",
        "src-tauri/crates/yss-math/src/adapter.rs",
        "src-tauri/crates/yss-math/src/ir.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "math parser owner must exist at {relative}"
        );
    }
    assert!(
        !root.join("src-tauri/src/math").exists(),
        "the root crate must not retain a math compatibility module"
    );
}

#[test]
fn variable_contract_has_one_pure_crate_owner_without_compatibility_module() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-variable-contract/src/lib.rs",
        "src-tauri/crates/yss-variable-contract/src/variable_id.rs",
        "src-tauri/crates/yss-variable-contract/src/variable_instance.rs",
        "src-tauri/crates/yss-variable-contract/src/variable_scope.rs",
        "src-tauri/crates/yss-variable-contract/tests/wire_contract.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "variable contract owner must exist at {relative}"
        );
    }
    assert!(
        !root.join("src-tauri/src/variable").exists(),
        "the root crate must not retain a variable compatibility module"
    );
}

#[test]
fn window_state_has_one_platform_crate_owner_without_compatibility_module() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-window-state/Cargo.toml",
        "src-tauri/crates/yss-window-state/src/lib.rs",
        "src-tauri/crates/yss-window-state/src/error.rs",
        "src-tauri/crates/yss-window-state/src/kind.rs",
        "src-tauri/crates/yss-window-state/src/persistence.rs",
        "src-tauri/crates/yss-window-state/src/tests.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "window state owner must exist at {relative}"
        );
    }
    assert!(
        !root.join("src-tauri/src/window_state").exists(),
        "the root crate must not retain a window state compatibility module"
    );
}

#[test]
fn project_watcher_ownership_is_neutral_and_platform_only() {
    let facts = production_facts();
    for source in [
        "src-tauri/src/platform/mod.rs",
        "src-tauri/src/platform/project_file_watcher.rs",
    ] {
        assert_eq!(
            facts.classification.get(source),
            Some(&RustLayer::PlatformAdapter),
            "watcher platform source must use the exact Platform classification"
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
            "src-tauri/src/sci/api/computation.rs",
        ],
    };

    assert!(canonical_owner_origins_are_valid(
        &expectation,
        &BTreeSet::from([
            "src-tauri/crates/yss-data-contract/src/data_value.rs",
            "src-tauri/src/sci/api/computation.rs",
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
        &BTreeSet::from(["src-tauri/src/sci/api/computation.rs"]),
    ));
}

#[test]
fn task1_sci_contracts_are_isolated_and_canonical() {
    const TASK1_SCI_FILES: &[&str] = &[
        "src-tauri/src/sci/api/computation.rs",
        "src-tauri/src/sci/api/node_statistics.rs",
        "src-tauri/src/sci/api/time_series/acf_pacf.rs",
        "src-tauri/src/sci/api/time_series/serial_tests.rs",
        "src-tauri/src/sci/backends/rust/stats/hypothesis.rs",
        "src-tauri/src/sci/backends/rust/time_series/acf_pacf.rs",
        "src-tauri/src/sci/error.rs",
        "src-tauri/src/sci/models/regression.rs",
    ];

    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let workspace = super::cargo_targets::discover_rust_workspace_model(&manifest)
        .expect("the real Cargo workspace must be discoverable");
    let modules = collect_production_modules(&workspace.repository_root, &workspace.roots)
        .expect("the production module graph must be discoverable");
    let classification = classify_rust_sources(&workspace.roots, &modules)
        .expect("every production source must classify exactly once");
    let raw_dependencies =
        collect_production_dependencies(&workspace.repository_root, &workspace.roots)
            .expect("production dependency facts must be discoverable");
    let dependencies = resolve_canonical_dependencies_detailed(&workspace, &raw_dependencies)
        .expect("production dependency origins must resolve");

    let forbidden = dependencies
        .iter()
        .filter(|dependency| TASK1_SCI_FILES.contains(&dependency.source_file.as_str()))
        .filter(|dependency| match &dependency.origin {
            CanonicalOrigin::Repository {
                repository_relative_declaration_file,
                ..
            } => matches!(
                classification.get(repository_relative_declaration_file),
                Some(RustLayer::Graph | RustLayer::Project | RustLayer::Execution)
            ),
            CanonicalOrigin::External(origin) => origin.package_name == "tauri",
            CanonicalOrigin::LanguageBuiltin { .. } | CanonicalOrigin::RepositoryAsset { .. } => {
                false
            }
        })
        .map(|dependency| {
            format!(
                "{}|{}",
                dependency.source_file, dependency.canonical_origin_target
            )
        })
        .collect::<Vec<_>>();
    assert!(
        forbidden.is_empty(),
        "Task 1 SCI contracts must not import Graph, Project, Execution, or Tauri: {forbidden:#?}"
    );

    for (symbol, expected_origins) in [
        (
            "CategoricalRole",
            BTreeSet::from([
                "src-tauri/crates/yss-data-contract/src/data_value.rs",
                "src-tauri/src/sci/api/computation.rs",
            ]),
        ),
        (
            "StatisticalObservationMetadata",
            BTreeSet::from(["src-tauri/src/sci/api/computation.rs"]),
        ),
        (
            "StatisticalSettingSource",
            BTreeSet::from(["src-tauri/src/sci/api/computation.rs"]),
        ),
    ] {
        let actual_origins = dependencies
            .iter()
            .filter_map(|dependency| match &dependency.origin {
                CanonicalOrigin::Repository {
                    repository_relative_declaration_file,
                    symbol: origin_symbol,
                    ..
                } if origin_symbol == symbol => Some(repository_relative_declaration_file.as_str()),
                CanonicalOrigin::Repository { .. }
                | CanonicalOrigin::LanguageBuiltin { .. }
                | CanonicalOrigin::RepositoryAsset { .. }
                | CanonicalOrigin::External(_) => None,
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            actual_origins, expected_origins,
            "{symbol} must have only its approved canonical owner(s)"
        );
    }

    let computation_data_value_origins = dependencies
        .iter()
        .filter_map(|dependency| match &dependency.origin {
            CanonicalOrigin::Repository {
                repository_relative_declaration_file,
                symbol,
                ..
            } if dependency.source_file == "src-tauri/src/sci/api/computation.rs"
                && symbol == "DataValue" =>
            {
                Some(repository_relative_declaration_file.as_str())
            }
            CanonicalOrigin::Repository { .. }
            | CanonicalOrigin::LanguageBuiltin { .. }
            | CanonicalOrigin::RepositoryAsset { .. }
            | CanonicalOrigin::External(_) => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        computation_data_value_origins,
        BTreeSet::from(["src-tauri/crates/yss-data-contract/src/data_value.rs"]),
        "StatisticalInputSource values must borrow the persisted DataValue owner"
    );
    let computation_persisted_role_dependencies = dependencies
        .iter()
        .filter(|dependency| {
            dependency.source_file == "src-tauri/src/sci/api/computation.rs"
                && matches!(
                    &dependency.origin,
                    CanonicalOrigin::Repository {
                        repository_relative_declaration_file,
                        symbol,
                        ..
                    } if repository_relative_declaration_file
                        == "src-tauri/crates/yss-data-contract/src/data_value.rs"
                        && symbol == "CategoricalRole"
                )
        })
        .collect::<Vec<_>>();
    assert!(
        computation_persisted_role_dependencies.is_empty(),
        "StatisticalInputSource must expose the SCI-owned CategoricalRole"
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
    fixture.write("src-tauri/src/lib.rs", "pub mod graph;\npub mod sci;\n");
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
    fixture.write("src-tauri/src/sci/mod.rs", "pub mod api;\n");
    fixture.write("src-tauri/src/sci/api/mod.rs", "pub mod computation;\n");
    fixture.write(
        "src-tauri/src/sci/api/computation.rs",
        r#"
pub enum CategoricalRole {
    Individual,
}

pub type PersistedCategoricalRole = yss_data_contract::CategoricalRole;
"#,
    );
    let modules = collect_production_modules(
        &fixture.root,
        &[ProductionRoot {
            package_id: "fixture-package".to_owned(),
            package: "fixture".to_owned(),
            target: "fixture_lib".to_owned(),
            kind: ProductionRootKind::Library,
            source_path: fixture.root.join("src-tauri/src/lib.rs"),
        }],
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
            "src-tauri/src/graph/value/aliases.rs|PersistedCategoricalRole|yss_data_contract::CategoricalRole",
            "src-tauri/src/graph/value/aliases.rs|PersistedDataSeriesValue|yss_data_contract::DataSeriesValue",
            "src-tauri/src/graph/value/aliases.rs|PersistedDataType|yss_data_contract::DataType",
            "src-tauri/src/graph/value/aliases.rs|PersistedDataValue|yss_data_contract::DataValue",
            "src-tauri/src/graph/value/aliases.rs|PersistedDummyInfo|yss_data_contract::DummyInfo",
            "src-tauri/src/graph/value/aliases.rs|PersistedTimeSeriesState|yss_data_contract::TimeSeriesState",
            "src-tauri/src/sci/api/computation.rs|PersistedCategoricalRole|yss_data_contract::CategoricalRole",
        ]
    );
}

#[test]
fn rust_build_script_and_external_dependency_policy_is_fail_closed() {
    const INTERNAL_CAPABILITIES: &[InternalDependencyCapability] =
        &[InternalDependencyCapability {
            source_layer: RustLayer::Commands,
            repository_relative_source_file: "src-tauri/src/commands/mod.rs",
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
            "src-tauri/src/commands/mod.rs".to_owned(),
            RustLayer::Commands,
        ),
        ("src-tauri/src/graph/mod.rs".to_owned(), RustLayer::Graph),
        (
            "src-tauri/src/application/mod.rs".to_owned(),
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
        "src-tauri/src/commands/mod.rs",
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
        source_file: "src-tauri/src/application/mod.rs".to_owned(),
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
        source_file: "src-tauri/src/commands/mod.rs".to_owned(),
        owner: "fixture_lib::commands".to_owned(),
        kind: RustDependencyKind::Path,
        mode: RustDependencyMode::Runtime,
        origin: CanonicalOrigin::Repository {
            package_name: "fixture".to_owned(),
            repository_relative_declaration_file: "src-tauri/src/application/mod.rs".to_owned(),
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
            repository_relative_source_file: "src-tauri/src/commands/*",
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
            repository_relative_declaration_file: "src-tauri/src/application/mod.rs".to_owned(),
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
