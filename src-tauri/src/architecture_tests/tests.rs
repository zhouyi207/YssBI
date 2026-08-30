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
    assert!(workspace.roots.iter().any(|root| root.package == "yss-math"
        && root.target == "yss_math"
        && root.kind == ProductionRootKind::Library));
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
                    && alias.declared_name == "yss_canonical_hash"
                    && alias.member_package == "yss-canonical-hash"
            })
    );
    assert!(workspace.dependency_declarations.iter().any(|dependency| {
        dependency.owning_package == "yssbi"
            && dependency.package_name == "yss-canonical-hash"
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
                    && alias.declared_name == "yss_execution"
                    && alias.member_package == "yss-execution"
            })
    );
    assert!(workspace.dependency_declarations.iter().any(|dependency| {
        dependency.owning_package == "yssbi"
            && dependency.package_name == "yss-execution"
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
                    && alias.declared_name == "yss_graph_analysis"
                    && alias.member_package == "yss-graph-analysis"
            })
    );
    assert!(workspace.dependency_declarations.iter().any(|dependency| {
        dependency.owning_package == "yssbi"
            && dependency.package_name == "yss-graph-analysis"
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
                    && alias.declared_name == "yss_graph_analysis_contract"
                    && alias.member_package == "yss-graph-analysis-contract"
            })
    );
    assert!(workspace.dependency_declarations.iter().any(|dependency| {
        dependency.owning_package == "yssbi"
            && dependency.package_name == "yss-graph-analysis-contract"
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
                    && alias.declared_name == "yss_graph_catalog"
                    && alias.member_package == "yss-graph-catalog"
            })
    );
    assert!(workspace.dependency_declarations.iter().any(|dependency| {
        dependency.owning_package == "yssbi"
            && dependency.package_name == "yss-graph-catalog"
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
                    && alias.declared_name == "yss_graph_compiler"
                    && alias.member_package == "yss-graph-compiler"
            })
    );
    assert!(workspace.dependency_declarations.iter().any(|dependency| {
        dependency.owning_package == "yssbi"
            && dependency.package_name == "yss-graph-compiler"
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
                    && alias.declared_name == "yss_graph_compiler_diagnostics"
                    && alias.member_package == "yss-graph-compiler-diagnostics"
            })
    );
    assert!(workspace.dependency_declarations.iter().any(|dependency| {
        dependency.owning_package == "yssbi"
            && dependency.package_name == "yss-graph-compiler-diagnostics"
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
                    && alias.declared_name == "yss_graph_document_edit"
                    && alias.member_package == "yss-graph-document-edit"
            })
    );
    assert!(workspace.dependency_declarations.iter().any(|dependency| {
        dependency.owning_package == "yssbi"
            && dependency.package_name == "yss-graph-document-edit"
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
                    && alias.declared_name == "yss_graph_resource_contract"
                    && alias.member_package == "yss-graph-resource-contract"
            })
    );
    assert!(workspace.dependency_declarations.iter().any(|dependency| {
        dependency.owning_package == "yssbi"
            && dependency.package_name == "yss-graph-resource-contract"
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
                    && alias.declared_name == "yss_graph_type_mapping"
                    && alias.member_package == "yss-graph-type-mapping"
            })
    );
    assert!(workspace.dependency_declarations.iter().any(|dependency| {
        dependency.owning_package == "yssbi"
            && dependency.package_name == "yss-graph-type-mapping"
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
                    && alias.declared_name == "yss_graph_registry"
                    && alias.member_package == "yss-graph-registry"
            })
    );
    assert!(workspace.dependency_declarations.iter().any(|dependency| {
        dependency.owning_package == "yssbi"
            && dependency.package_name == "yss-graph-registry"
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
                    && alias.declared_name == "yss_path_display"
                    && alias.member_package == "yss-path-display"
            })
    );
    assert!(workspace.dependency_declarations.iter().any(|dependency| {
        dependency.owning_package == "yssbi"
            && dependency.package_name == "yss-path-display"
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
                    && alias.declared_name == "yss_project_model"
                    && alias.member_package == "yss-project-model"
            })
    );
    assert!(workspace.dependency_declarations.iter().any(|dependency| {
        dependency.owning_package == "yssbi"
            && dependency.package_name == "yss-project-model"
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
        package: "yss-math".to_owned(),
        target: "yss_math".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-math/src/lib.rs"),
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
        package: "yss-project-manifest".to_owned(),
        target: "yss_project_manifest".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-project-manifest/src/lib.rs"),
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
    let variable_contract_root = ProductionRoot {
        package_id: "variable-contract-package".to_owned(),
        package: "yss-variable-contract".to_owned(),
        target: "yss_variable_contract".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-variable-contract/src/lib.rs"),
    };
    let variable_value_root = ProductionRoot {
        package_id: "variable-value-package".to_owned(),
        package: "yss-variable-value".to_owned(),
        target: "yss_variable_value".to_owned(),
        kind: ProductionRootKind::Library,
        source_path: PathBuf::from("src-tauri/crates/yss-variable-value/src/lib.rs"),
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
        variable_contract_root.clone(),
        variable_value_root.clone(),
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
                "src-tauri/crates/yss-math/src/lib.rs",
                "yss_math",
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
                "src-tauri/crates/yss-project-manifest/src/lib.rs",
                "yss_project_manifest",
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
                &variable_contract_root,
                "src-tauri/crates/yss-variable-contract/src/lib.rs",
                "yss_variable_contract",
            ),
            module(
                &variable_value_root,
                "src-tauri/crates/yss-variable-value/src/lib.rs",
                "yss_variable_value",
            ),
            module(
                &window_state_root,
                "src-tauri/crates/yss-window-state/src/lib.rs",
                "yss_window_state",
            ),
            module(
                &execution_root,
                "src-tauri/crates/yss-execution/src/settings.rs",
                "yss_execution::settings",
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
        classified["src-tauri/crates/yss-math/src/lib.rs"],
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
        classified["src-tauri/crates/yss-project-manifest/src/lib.rs"],
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
        classified["src-tauri/crates/yss-variable-contract/src/lib.rs"],
        RustLayer::PureLeaf
    );
    assert_eq!(
        classified["src-tauri/crates/yss-variable-value/src/lib.rs"],
        RustLayer::PureLeaf
    );
    assert_eq!(
        classified["src-tauri/crates/yss-window-state/src/lib.rs"],
        RustLayer::PlatformAdapter
    );
    assert_eq!(
        classified["src-tauri/crates/yss-execution/src/settings.rs"],
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
        "src-tauri/crates/yss-graph-document/src/resource_path.rs",
        "src-tauri/crates/yss-resource-naming/src/lib.rs",
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
        std::fs::read_to_string(root.join("src-tauri/src/project/project_store.rs"))
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
fn execution_has_one_crate_owner_without_compatibility_or_dead_effect_mirrors() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-execution/Cargo.toml",
        "src-tauri/crates/yss-execution/src/lib.rs",
        "src-tauri/crates/yss-execution/src/plan/mod.rs",
        "src-tauri/crates/yss-execution/src/state.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "execution owner must exist at {relative}"
        );
    }
    assert!(
        !root.join("src-tauri/src/execution").exists(),
        "the root crate must not retain an execution compatibility module"
    );

    let manifest = std::fs::read_to_string(root.join("src-tauri/crates/yss-execution/Cargo.toml"))
        .expect("execution manifest must be readable");
    assert!(
        manifest.contains("test-support = []"),
        "execution test constructors must stay behind an explicit feature"
    );

    let sources = [
        "src-tauri/crates/yss-execution/src/finalization.rs",
        "src-tauri/crates/yss-execution/src/resource_preparation.rs",
        "src-tauri/crates/yss-execution/src/state.rs",
    ]
    .into_iter()
    .map(|relative| {
        std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("{relative} must be readable: {error}"))
    })
    .collect::<String>();
    for removed in [
        "CandidateEffectProjection",
        "CandidateExecutionEffects",
        "ExecutionEffectBuffer",
        "enum ExecutionEffect",
        "cfg(all(test, any()))",
    ] {
        assert!(
            !sources.contains(removed),
            "execution must not restore dead effect/test machinery '{removed}'"
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
fn graph_type_mapping_has_one_pure_crate_owner_without_match_table_mirrors() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-graph-type-mapping/Cargo.toml",
        "src-tauri/crates/yss-graph-type-mapping/src/lib.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "graph type mapping owner must exist at {relative}"
        );
    }

    let manifest = root.join("src-tauri/Cargo.toml");
    let workspace = super::cargo_targets::discover_rust_workspace_model(&manifest)
        .expect("the real Cargo workspace must be discoverable");
    let modules = collect_production_modules(&workspace.repository_root, &workspace.roots)
        .expect("production modules must be discoverable");
    let duplicate_match_tables = modules
        .iter()
        .filter(|module| {
            module.repository_relative_source_file
                != "src-tauri/crates/yss-graph-type-mapping/src/lib.rs"
        })
        .filter_map(|module| {
            let source = std::fs::read_to_string(
                workspace
                    .repository_root
                    .join(&module.repository_relative_source_file),
            )
            .ok()?;
            (source.contains("DataType::Boolean =>")
                && source.contains("DataType::DataSeries")
                && source.contains("TypeExpr::Unknown"))
            .then_some(module.repository_relative_source_file.clone())
        })
        .collect::<Vec<_>>();
    assert!(
        duplicate_match_tables.is_empty(),
        "DataType to TypeExpr mapping must have one owner: {duplicate_match_tables:#?}"
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
fn variable_value_has_one_pure_owner_without_project_mirrors_or_silent_activation_errors() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-variable-value/Cargo.toml",
        "src-tauri/crates/yss-variable-value/src/lib.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "variable-value owner must exist at {relative}"
        );
    }
    for removed_owner in [
        "src-tauri/src/project/variable_defaults.rs",
        "src-tauri/src/project/variable_tabular.rs",
    ] {
        assert!(
            !root.join(removed_owner).exists(),
            "Project must not retain removed variable-value owner {removed_owner}"
        );
    }

    let workspace_manifest = std::fs::read_to_string(root.join("src-tauri/Cargo.toml"))
        .expect("the Rust workspace manifest must be readable");
    for declaration in [
        "\"crates/yss-variable-value\"",
        "yss-variable-value = { path = \"./crates/yss-variable-value\" }",
    ] {
        assert!(
            workspace_manifest.contains(declaration),
            "the workspace and root package must declare {declaration}"
        );
    }

    let manifest =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-variable-value/Cargo.toml"))
            .expect("variable-value crate manifest must be readable");
    for dependency in [
        "yss-data-contract = { path = \"../yss-data-contract\" }",
        "yss-tabular-contract = { path = \"../yss-tabular-contract\" }",
        "yss-variable-contract = { path = \"../yss-variable-contract\" }",
    ] {
        assert!(
            manifest.contains(dependency),
            "variable-value must consume canonical dependency {dependency}"
        );
    }
    for forbidden in ["tauri", "sqlx", "polars", "chrono"] {
        assert!(
            !manifest.contains(forbidden),
            "variable-value must not absorb runtime dependency '{forbidden}'"
        );
    }

    let owner =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-variable-value/src/lib.rs"))
            .expect("variable-value owner must be readable");
    for invariant in [
        "pub fn default_value_for",
        "DataType::Array(_) => DataValue::Array(Vec::new())",
        "pub enum VariableTabularNormalizationError",
        "MissingSnapshot",
        "pub fn variable_handle",
        "pub fn normalize_variable_tabular",
        "value.id = variable_handle(&variable.id)",
    ] {
        assert!(
            owner.contains(invariant),
            "variable-value crate must own invariant '{invariant}'"
        );
    }
    for misplaced_concern in [
        "std::fs",
        "ProjectState",
        "ProjectData",
        "tauri::",
        "sqlx::",
        "polars::",
    ] {
        assert!(
            !owner.contains(misplaced_concern),
            "variable-value must not absorb runtime concern '{misplaced_concern}'"
        );
    }

    let project_module = std::fs::read_to_string(root.join("src-tauri/src/project/mod.rs"))
        .expect("the root project module must be readable");
    for facade in [
        "mod variable_defaults",
        "mod variable_tabular",
        "pub use yss_variable_value",
    ] {
        assert!(
            !project_module.contains(facade),
            "the root project module must not restore compatibility facade '{facade}'"
        );
    }

    for relative in [
        "src-tauri/src/project/project_activation.rs",
        "src-tauri/src/project/project_state_variable.rs",
        "src-tauri/src/project/project_writers.rs",
    ] {
        let consumer = std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("{relative} must be readable: {error}"));
        assert!(
            consumer.contains("yss_variable_value"),
            "{relative} must consume the canonical variable-value owner directly"
        );
        assert!(
            !consumer.contains("crate::project::variable_defaults")
                && !consumer.contains("crate::project::variable_tabular"),
            "{relative} must not restore removed Project variable-value paths"
        );
    }

    let activation =
        std::fs::read_to_string(root.join("src-tauri/src/project/project_activation.rs"))
            .expect("project activation must be readable");
    assert!(
        !activation.contains("let _ = normalize_variable_tabular"),
        "project activation must not silently discard variable normalization failures"
    );
    assert!(
        activation.contains("ProjectFilesystemError::TransactionPrepareFailed"),
        "project activation must fail closed during invalid variable preparation"
    );

    let policy = std::fs::read_to_string(root.join("src-tauri/src/architecture_tests/policy.rs"))
        .expect("Rust architecture policy must be readable");
    assert!(
        policy.contains("| \"yss-variable-value\"")
            && policy.contains("layers.insert(RustLayer::PureLeaf)"),
        "variable-value must remain a Pure Leaf"
    );
}

#[test]
fn path_display_has_one_dependency_free_pure_owner_without_project_facade() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-path-display/Cargo.toml",
        "src-tauri/crates/yss-path-display/src/lib.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "path-display owner must exist at {relative}"
        );
    }
    assert!(
        !root.join("src-tauri/src/project/path_format.rs").exists(),
        "Project must not retain the removed path display owner"
    );

    let workspace_manifest = std::fs::read_to_string(root.join("src-tauri/Cargo.toml"))
        .expect("the Rust workspace manifest must be readable");
    for declaration in [
        "\"crates/yss-path-display\"",
        "yss-path-display = { path = \"./crates/yss-path-display\" }",
    ] {
        assert!(
            workspace_manifest.contains(declaration),
            "the workspace and root package must declare {declaration}"
        );
    }

    let manifest =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-path-display/Cargo.toml"))
            .expect("path-display crate manifest must be readable");
    assert!(
        !manifest.contains("dependencies"),
        "path-display must remain dependency-free"
    );

    let owner = std::fs::read_to_string(root.join("src-tauri/crates/yss-path-display/src/lib.rs"))
        .expect("path-display owner must be readable");
    for invariant in [
        "pub fn format_path_for_user",
        "pub fn format_path_for_user_path",
        r#"path.strip_prefix(r"\\?\UNC\")"#,
        r#"path.strip_prefix(r"\\?\")"#,
    ] {
        assert!(
            owner.contains(invariant),
            "path-display crate must own invariant '{invariant}'"
        );
    }
    for misplaced_concern in [
        "#[cfg(windows)]",
        "std::fs",
        "ProjectState",
        "ProjectData",
        "tauri::",
        "sqlx::",
    ] {
        assert!(
            !owner.contains(misplaced_concern),
            "path-display must not absorb runtime concern '{misplaced_concern}'"
        );
    }

    for relative in [
        "src-tauri/src/application/project_query.rs",
        "src-tauri/crates/yss-project-registry/src/lib.rs",
    ] {
        let consumer = std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("{relative} must be readable: {error}"));
        assert!(
            consumer.contains("yss_path_display"),
            "{relative} must consume the canonical path-display owner directly"
        );
        assert!(
            !consumer.contains("crate::project::path_format"),
            "{relative} must not restore the removed Project path-format path"
        );
    }

    let project_module = std::fs::read_to_string(root.join("src-tauri/src/project/mod.rs"))
        .expect("the root project module must be readable");
    for facade in ["mod path_format", "pub use yss_path_display"] {
        assert!(
            !project_module.contains(facade),
            "the root project module must not restore compatibility facade '{facade}'"
        );
    }

    let policy = std::fs::read_to_string(root.join("src-tauri/src/architecture_tests/policy.rs"))
        .expect("Rust architecture policy must be readable");
    assert!(
        policy.contains("| \"yss-path-display\"")
            && policy.contains("layers.insert(RustLayer::PureLeaf)"),
        "path-display must remain a Pure Leaf"
    );
    assert!(
        !policy.contains("yssbi_lib::project::path_format::format_path_for_user_path"),
        "architecture capabilities must not preserve the removed Project facade"
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
fn canonical_hash_has_one_pure_crate_owner_without_registry_duplication() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-canonical-hash/Cargo.toml",
        "src-tauri/crates/yss-canonical-hash/src/lib.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "canonical hash owner must exist at {relative}"
        );
    }

    let registry_fingerprint = std::fs::read_to_string(
        root.join("src-tauri/crates/yss-graph-registry/src/fingerprint.rs"),
    )
    .expect("registry fingerprint source must be readable");
    assert!(
        !registry_fingerprint.contains("fn sha256"),
        "graph registry must consume yss-canonical-hash instead of retaining a SHA-256 implementation"
    );
}

#[test]
fn graph_registry_has_one_graph_crate_owner_without_compatibility_module() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-graph-registry/Cargo.toml",
        "src-tauri/crates/yss-graph-registry/src/fingerprint.rs",
        "src-tauri/crates/yss-graph-registry/src/lib.rs",
        "src-tauri/crates/yss-graph-registry/src/model.rs",
        "src-tauri/crates/yss-graph-registry/src/validation.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "graph registry owner must exist at {relative}"
        );
    }
    assert!(
        !root.join("src-tauri/src/graph/registry").exists(),
        "the root crate must not retain a graph registry compatibility module"
    );

    let sources = [
        "src-tauri/crates/yss-graph-registry/src/lib.rs",
        "src-tauri/crates/yss-graph-registry/src/model.rs",
        "src-tauri/crates/yss-graph-registry/src/validation.rs",
        "src-tauri/crates/yss-graph-catalog/src/builtin.rs",
        "src-tauri/crates/yss-graph-catalog/src/dataframe/mod.rs",
    ]
    .into_iter()
    .map(|relative| {
        std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("{relative} must be readable: {error}"))
    })
    .collect::<String>();
    for removed in [
        "PreparedNominalValue",
        "NominalValueHandle",
        "register_nominal_codec",
        "NodeImplementationCapability",
        "NodeImplementation",
        "ImplementationKind",
        "cfg(all(test, any()))",
    ] {
        assert!(
            !sources.contains(removed),
            "graph registry/catalog must not restore removed dead API '{removed}'"
        );
    }
}

#[test]
fn graph_analysis_has_one_behavior_owner_without_noop_context_inputs() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-graph-analysis/Cargo.toml",
        "src-tauri/crates/yss-graph-analysis/src/lib.rs",
        "src-tauri/crates/yss-graph-analysis/src/result_category.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "graph analysis behavior owner must exist at {relative}"
        );
    }
    for removed in [
        "src-tauri/src/graph/analysis",
        "src-tauri/src/graph/settings.rs",
        "src-tauri/crates/yss-graph-analysis/src/settings.rs",
    ] {
        assert!(
            !root.join(removed).exists(),
            "graph analysis must not retain compatibility or no-op settings owner {removed}"
        );
    }

    let source =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-graph-analysis/src/lib.rs"))
            .expect("graph analysis source must be readable");
    for removed in [
        "GraphCompileSettings",
        "ResourceCatalogSnapshot",
        "input.catalog",
        "input.settings",
    ] {
        assert!(
            !source.contains(removed),
            "graph analysis must not restore ignored context input '{removed}'"
        );
    }
}

#[test]
fn graph_compiler_has_one_owner_without_optional_or_mirrored_compile_state() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-graph-compiler/Cargo.toml",
        "src-tauri/crates/yss-graph-compiler/src/compiler.rs",
        "src-tauri/crates/yss-graph-compiler/src/error.rs",
        "src-tauri/crates/yss-graph-compiler/src/lib.rs",
        "src-tauri/crates/yss-graph-compiler/src/package.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "graph compiler owner must exist at {relative}"
        );
    }
    assert!(
        !root.join("src-tauri/src/graph/compiler").exists(),
        "the root crate must not retain a graph compiler compatibility module"
    );

    let compiler_sources = [
        "src-tauri/crates/yss-graph-compiler/src/compiler.rs",
        "src-tauri/crates/yss-graph-compiler/src/error.rs",
        "src-tauri/crates/yss-graph-compiler/src/lib.rs",
        "src-tauri/crates/yss-graph-compiler/src/package.rs",
    ]
    .into_iter()
    .map(|relative| {
        std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("{relative} must be readable: {error}"))
    })
    .collect::<String>();
    for removed in [
        "CompilationReport",
        "GraphDiagnostic",
        "Option<GraphCompiledPackage>",
        "CompilationBasis",
        "GraphAnalysisInput",
        "AnalysisInvariant",
        "GraphCompileSource",
        "GraphCompileError::Catalog",
        "GraphCompileError::Internal",
    ] {
        assert!(
            !compiler_sources.contains(removed),
            "graph compiler must not restore no-op or zero-producer state {removed}"
        );
    }

    assert!(
        !root.join("src-tauri/src/graph/error.rs").exists(),
        "the root crate must not restore a second graph error owner"
    );
    let run_graph =
        std::fs::read_to_string(root.join("src-tauri/src/application/execution/run_graph.rs"))
            .expect("run graph source must be readable");
    assert!(
        !run_graph.contains("PackageUnavailable"),
        "an infallible optional-package branch must not return"
    );
}

#[test]
fn graph_analysis_contract_has_one_graph_crate_owner_without_compatibility_module() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-graph-analysis-contract/Cargo.toml",
        "src-tauri/crates/yss-graph-analysis-contract/src/basis.rs",
        "src-tauri/crates/yss-graph-analysis-contract/src/diagnostic.rs",
        "src-tauri/crates/yss-graph-analysis-contract/src/lib.rs",
        "src-tauri/crates/yss-graph-analysis-contract/src/provenance.rs",
        "src-tauri/crates/yss-graph-analysis-contract/src/semantic.rs",
        "src-tauri/crates/yss-graph-analysis-contract/src/snapshot.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "graph analysis contract owner must exist at {relative}"
        );
    }
    assert!(
        !root.join("src-tauri/src/graph/analysis/contracts").exists(),
        "the root crate must not retain a graph analysis contract compatibility module"
    );

    let sources = [
        "src-tauri/crates/yss-graph-analysis-contract/src/basis.rs",
        "src-tauri/crates/yss-graph-analysis-contract/src/diagnostic.rs",
        "src-tauri/crates/yss-graph-analysis-contract/src/lib.rs",
        "src-tauri/crates/yss-graph-analysis-contract/src/provenance.rs",
        "src-tauri/crates/yss-graph-analysis-contract/src/semantic.rs",
        "src-tauri/crates/yss-graph-analysis-contract/src/snapshot.rs",
    ]
    .into_iter()
    .map(|relative| {
        std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("{relative} must be readable: {error}"))
    })
    .collect::<String>();
    for removed in [
        "AnalysisResourceResolver",
        "ResolvedFunctionValue",
        "CompileProjection",
        "pub type Severity",
        "pub type Location",
        "fn unknown(",
        "has_blocking_errors",
    ] {
        assert!(
            !sources.contains(removed),
            "graph analysis contract must not restore removed dead API '{removed}'"
        );
    }

    assert!(
        !root.join("src-tauri/src/graph/compatibility.rs").exists(),
        "analysis compatibility must not restore the removed root graph compatibility owner"
    );
}

#[test]
fn graph_compiler_diagnostics_has_one_graph_crate_owner_without_dead_constructor_api() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-graph-compiler-diagnostics/Cargo.toml",
        "src-tauri/crates/yss-graph-compiler-diagnostics/src/lib.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "graph compiler diagnostics owner must exist at {relative}"
        );
    }
    assert!(
        !root
            .join("src-tauri/src/graph/catalog/diagnostics.rs")
            .exists(),
        "the root crate must not retain a graph compiler diagnostics compatibility module"
    );

    let source = std::fs::read_to_string(
        root.join("src-tauri/crates/yss-graph-compiler-diagnostics/src/lib.rs"),
    )
    .expect("graph compiler diagnostics source must be readable");
    for required in [
        "pub const COMPILER_DIAGNOSTIC_DEFINITIONS",
        "pub fn validate_compiler_diagnostic_definitions",
    ] {
        assert!(
            source.contains(required),
            "graph compiler diagnostics must retain authoritative API '{required}'"
        );
    }
    for removed in [
        "pub enum CompilerDiagnostic {",
        "CompilerNodeDiagnostic",
        "resource_resolution_failed",
        "compare_diagnostics",
        "managed_node_role_name",
        "node_scope_name",
        "port_kind_name",
        "tracing::warn!",
        "uuid::",
    ] {
        assert!(
            !source.contains(removed),
            "graph compiler diagnostics must not restore removed dead API '{removed}'"
        );
    }
}

#[test]
fn graph_catalog_has_one_crate_owner_with_explicit_test_support_boundary() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-graph-catalog/Cargo.toml",
        "src-tauri/crates/yss-graph-catalog/src/builtin.rs",
        "src-tauri/crates/yss-graph-catalog/src/dataframe/mod.rs",
        "src-tauri/crates/yss-graph-catalog/src/lib.rs",
        "src-tauri/crates/yss-graph-catalog/src/localization.rs",
        "src-tauri/crates/yss-graph-catalog/src/project.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "graph catalog owner must exist at {relative}"
        );
    }
    assert!(
        !root.join("src-tauri/src/graph/catalog").exists(),
        "the root crate must not retain a graph catalog compatibility module"
    );
    assert!(
        !root
            .join("src-tauri/crates/yss-graph-catalog/src/project_interface.rs")
            .exists(),
        "the orphan project interface duplicate must not return"
    );

    let manifest =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-graph-catalog/Cargo.toml"))
            .expect("graph catalog manifest must be readable");
    assert!(
        manifest.contains("test-support = []"),
        "graph catalog test hooks must stay behind an explicit feature"
    );

    let sources = [
        "src-tauri/crates/yss-graph-catalog/src/builtin.rs",
        "src-tauri/crates/yss-graph-catalog/src/lib.rs",
        "src-tauri/crates/yss-graph-catalog/src/localization.rs",
        "src-tauri/crates/yss-graph-catalog/src/plot/mod.rs",
        "src-tauri/crates/yss-graph-catalog/src/project.rs",
    ]
    .into_iter()
    .map(|relative| {
        std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("{relative} must be readable: {error}"))
    })
    .collect::<String>();
    for removed in [
        "cfg(all(test, any()))",
        "BuiltinAssemblyTestFault",
        "assemble_builtin_parts_with",
        "build_builtin_node_system_with_test_fault",
        "remove_message_for_test",
        "replace_message_for_test",
        "replace_text_for_test",
        "PlotLowerer",
        "PLOT_SINK",
    ] {
        assert!(
            !sources.contains(removed),
            "graph catalog must not restore removed dead API '{removed}'"
        );
    }
}

#[test]
fn graph_document_has_one_pure_crate_owner_without_compatibility_module() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-graph-document/Cargo.toml",
        "src-tauri/crates/yss-graph-document/src/identity.rs",
        "src-tauri/crates/yss-graph-document/src/lib.rs",
        "src-tauri/crates/yss-graph-document/src/model.rs",
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

    let manifest =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-graph-document/Cargo.toml"))
            .expect("graph document manifest must be readable");
    assert!(
        manifest.contains("yss-resource-naming"),
        "graph resource paths must consume the canonical resource-name owner"
    );
    for obsolete_dependency in ["regex.workspace", "unicode-normalization.workspace"] {
        assert!(
            !manifest.contains(obsolete_dependency),
            "graph document must not retain resource-name dependency {obsolete_dependency}"
        );
    }
}

#[test]
fn resource_naming_has_one_pure_crate_owner_without_root_or_graph_facades() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-resource-naming/Cargo.toml",
        "src-tauri/crates/yss-resource-naming/src/lib.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "resource-name owner must exist at {relative}"
        );
    }
    for obsolete in [
        "src-tauri/src/project/resource_name.rs",
        "src-tauri/crates/yss-graph-document/src/name.rs",
    ] {
        assert!(
            !root.join(obsolete).exists(),
            "obsolete resource-name owner must not return at {obsolete}"
        );
    }

    let project_module = std::fs::read_to_string(root.join("src-tauri/src/project/mod.rs"))
        .expect("project module must be readable");
    assert!(
        !project_module.contains("mod resource_name")
            && !project_module.contains("resource_name::*"),
        "project must consume yss-resource-naming directly without a compatibility facade"
    );

    let graph_document =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-graph-document/src/lib.rs"))
            .expect("graph document root must be readable");
    for compatibility_export in [
        "ResourceNameValidationError",
        "MAX_RESOURCE_NAME_CHARACTERS",
        "validate_resource_name",
    ] {
        assert!(
            !graph_document.contains(compatibility_export),
            "graph document must not re-export resource naming API {compatibility_export}"
        );
    }

    let owner =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-resource-naming/src/lib.rs"))
            .expect("resource-name owner must be readable");
    for canonical_api in [
        "pub struct ResourceName",
        "pub enum ResourceNameValidationError",
        "pub const MAX_RESOURCE_NAME_CHARACTERS",
        "pub fn validate_resource_name",
        "pub fn allocate_unique_resource_name",
    ] {
        assert!(
            owner.contains(canonical_api),
            "resource-name owner must expose canonical API {canonical_api}"
        );
    }
    assert!(
        !owner.contains("allocate_unique_display_name"),
        "strict filesystem resource naming must stay distinct from loose display-name allocation"
    );
}

#[test]
fn display_naming_has_one_pure_crate_owner_without_root_facade() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-display-naming/Cargo.toml",
        "src-tauri/crates/yss-display-naming/src/lib.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "display-name owner must exist at {relative}"
        );
    }
    for obsolete in [
        "src-tauri/src/project/unique_name.rs",
        "src/shared/utils/getUniqueName.ts",
    ] {
        assert!(
            !root.join(obsolete).exists(),
            "obsolete display-name owner must not return at {obsolete}"
        );
    }

    let shared_utils = std::fs::read_to_string(root.join("src/shared/utils/index.ts"))
        .expect("shared utility exports must be readable");
    assert!(
        !shared_utils.contains("getUniqueName"),
        "the unused frontend display-name compatibility export must stay removed"
    );

    let project_module = std::fs::read_to_string(root.join("src-tauri/src/project/mod.rs"))
        .expect("project module must be readable");
    assert!(
        !project_module.contains("mod unique_name"),
        "project must consume yss-display-naming directly without a compatibility facade"
    );

    let owner =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-display-naming/src/lib.rs"))
            .expect("display-name owner must be readable");
    assert!(
        owner.contains("pub fn allocate_unique_display_name"),
        "display-name allocation must have one explicit canonical API"
    );
    for forbidden in ["regex::", "ResourceName", "allocate_unique_resource_name"] {
        assert!(
            !owner.contains(forbidden),
            "loose display naming must not absorb strict resource-name fact {forbidden}"
        );
    }

    let strict_owner =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-resource-naming/src/lib.rs"))
            .expect("resource-name owner must be readable");
    assert!(
        !strict_owner.contains("allocate_unique_display_name"),
        "strict resource naming must not duplicate loose display-name allocation"
    );

    for consumer in [
        "src-tauri/src/application/database.rs",
        "src-tauri/src/project/project_state_variable.rs",
        "src-tauri/src/project/project_writers/variables.rs",
    ] {
        let source = std::fs::read_to_string(root.join(consumer)).unwrap_or_else(|error| {
            panic!("display-name consumer {consumer} must be readable: {error}")
        });
        assert!(
            source.contains("yss_display_naming"),
            "display-name consumer {consumer} must depend on the canonical crate directly"
        );
        assert!(
            !source.contains("project::unique_name")
                && !source.contains("unique_name::unique_name"),
            "display-name consumer {consumer} must not restore the root compatibility path"
        );
    }

    let root_manifest = std::fs::read_to_string(root.join("src-tauri/Cargo.toml"))
        .expect("root Cargo manifest must be readable");
    assert!(
        root_manifest.contains("yss-display-naming = { path = \"./crates/yss-display-naming\" }"),
        "the root crate must declare its direct display-name dependency"
    );
    assert!(
        !root_manifest
            .lines()
            .any(|line| line.trim() == "regex.workspace = true"),
        "the root crate must not retain regex after the display-name parser becomes dependency-free"
    );
}

#[test]
fn graph_document_edit_has_one_graph_crate_owner_without_root_reexports() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-graph-document-edit/Cargo.toml",
        "src-tauri/crates/yss-graph-document-edit/src/error.rs",
        "src-tauri/crates/yss-graph-document-edit/src/lib.rs",
        "src-tauri/crates/yss-graph-document-edit/src/patch.rs",
        "src-tauri/crates/yss-graph-document-edit/src/validation.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "graph document edit owner must exist at {relative}"
        );
    }
    for relative in [
        "src-tauri/src/graph/document",
        "src-tauri/src/graph/document/error.rs",
        "src-tauri/src/graph/document/patch.rs",
        "src-tauri/src/graph/document/transaction.rs",
    ] {
        assert!(
            !root.join(relative).exists(),
            "the root crate must not retain graph document edit owner {relative}"
        );
    }

    let validation = std::fs::read_to_string(
        root.join("src-tauri/crates/yss-graph-document-edit/src/validation.rs"),
    )
    .expect("the graph document validation owner must be readable");
    assert!(
        !validation.contains("address_is_complete"),
        "the zero-consumer address completeness helper must stay removed"
    );
}

#[test]
fn graph_editor_has_one_graph_crate_owner_without_root_compatibility_modules() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-graph-editor/Cargo.toml",
        "src-tauri/crates/yss-graph-editor/src/compatibility.rs",
        "src-tauri/crates/yss-graph-editor/src/lib.rs",
        "src-tauri/crates/yss-graph-editor/src/mutation.rs",
        "src-tauri/crates/yss-graph-editor/src/mutation/connection.rs",
        "src-tauri/crates/yss-graph-editor/src/subgraph.rs",
        "src-tauri/crates/yss-graph-editor/src/subgraph/clipboard.rs",
        "src-tauri/crates/yss-graph-editor/src/subgraph/instantiate.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "graph editor owner must exist at {relative}"
        );
    }
    for relative in [
        "src-tauri/src/graph/compatibility.rs",
        "src-tauri/src/graph/document",
        "src-tauri/src/graph/mutation.rs",
        "src-tauri/src/graph/node",
    ] {
        assert!(
            !root.join(relative).exists(),
            "the root crate must not retain graph editor compatibility owner {relative}"
        );
    }

    let manifest = std::fs::read_to_string(root.join("src-tauri/Cargo.toml"))
        .expect("the Rust workspace manifest must be readable");
    for declaration in [
        "\"crates/yss-graph-editor\"",
        "yss-graph-editor = { path = \"./crates/yss-graph-editor\" }",
    ] {
        assert!(
            manifest.contains(declaration),
            "the workspace and root package must declare {declaration}"
        );
    }
    let editor_manifest =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-graph-editor/Cargo.toml"))
            .expect("the graph editor manifest must be readable");
    assert!(
        editor_manifest.contains(
            "yss-graph-resource-contract = { path = \"../yss-graph-resource-contract\" }"
        ),
        "compatible-catalog filtering must consume the canonical resource contract"
    );

    let sources = [
        "src-tauri/crates/yss-graph-editor/src/compatibility.rs",
        "src-tauri/crates/yss-graph-editor/src/mutation.rs",
        "src-tauri/crates/yss-graph-editor/src/mutation/connection.rs",
        "src-tauri/crates/yss-graph-editor/src/subgraph.rs",
        "src-tauri/crates/yss-graph-editor/src/subgraph/clipboard.rs",
        "src-tauri/crates/yss-graph-editor/src/subgraph/instantiate.rs",
    ]
    .into_iter()
    .map(|relative| {
        std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("{relative} must be readable: {error}"))
    })
    .collect::<String>();
    for removed in [
        "cfg(all(test, any()))",
        "EditorMutationValidationSnapshot",
        "ProjectedConnectPlan",
        "RevisionedGraphStore",
        "fn from_projection(",
        "projected_connect_operations",
        "fn source_from_projection(",
        "parameter_binding",
        "allowed_node_type_id",
        ".parameters.values().find_map",
    ] {
        assert!(
            !sources.contains(removed),
            "graph editor must not restore removed dormant API '{removed}'"
        );
    }

    let runtime =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-graph-runtime/src/lib.rs"))
            .expect("the graph runtime owner must be readable");
    assert!(
        runtime.contains("filter_compatible_catalog"),
        "the graph runtime must delegate compatible-catalog filtering to yss-graph-editor"
    );
    for duplicate in [
        "fn source_port(",
        "fn candidate_ports(",
        "fn ports_are_compatible(",
        "fn is_unresolved(",
        "fn source_is_database(",
    ] {
        assert!(
            !runtime.contains(duplicate),
            "the graph runtime must not restore duplicate editor compatibility logic '{duplicate}'"
        );
    }
}

#[test]
fn graph_runtime_has_one_graph_crate_owner_without_root_facade_or_dead_state() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-graph-runtime/Cargo.toml",
        "src-tauri/crates/yss-graph-runtime/src/lib.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "graph runtime owner must exist at {relative}"
        );
    }
    assert!(
        !root.join("src-tauri/src/graph").exists(),
        "the root crate must not retain a graph runtime facade"
    );

    let manifest = std::fs::read_to_string(root.join("src-tauri/Cargo.toml"))
        .expect("the Rust workspace manifest must be readable");
    for declaration in [
        "\"crates/yss-graph-runtime\"",
        "yss-graph-runtime = { path = \"./crates/yss-graph-runtime\" }",
        "yss-graph-runtime = { path = \"./crates/yss-graph-runtime\", features = [\"test-support\"] }",
    ] {
        assert!(
            manifest.contains(declaration),
            "the workspace and root package must declare {declaration}"
        );
    }

    let runtime =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-graph-runtime/src/lib.rs"))
            .expect("the graph runtime owner must be readable");
    for removed in [
        "resource_catalog:",
        "fn accepts_basis(",
        "fn resource_catalog(",
        "bind_open_graph",
        "GraphRuntimeTestEvent::Bound",
        "pub(crate)",
    ] {
        assert!(
            !runtime.contains(removed),
            "the graph runtime must not restore dead or root-private API '{removed}'"
        );
    }
    assert!(
        runtime.contains("cfg(any(test, feature = \"test-support\"))"),
        "runtime fault injection must remain behind the explicit test-support boundary"
    );
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
fn project_identity_has_one_pure_crate_owner_without_root_facade() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-project-identity/Cargo.toml",
        "src-tauri/crates/yss-project-identity/src/identity.rs",
        "src-tauri/crates/yss-project-identity/src/lib.rs",
        "src-tauri/crates/yss-project-identity/src/project_instance_id.rs",
        "src-tauri/crates/yss-project-identity/src/project_registration_id.rs",
        "src-tauri/crates/yss-project-identity/src/project_root_identity.rs",
        "src-tauri/crates/yss-project-identity/src/project_session_id.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "project identity owner must exist at {relative}"
        );
    }
    for relative in [
        "src-tauri/src/project/identity.rs",
        "src-tauri/src/project/project_session_id.rs",
    ] {
        assert!(
            !root.join(relative).exists(),
            "the root crate must not retain project identity owner {relative}"
        );
    }

    let workspace_manifest = std::fs::read_to_string(root.join("src-tauri/Cargo.toml"))
        .expect("the Rust workspace manifest must be readable");
    for declaration in [
        "\"crates/yss-project-identity\"",
        "yss-project-identity = { path = \"./crates/yss-project-identity\" }",
        "yss-project-identity = { path = \"./crates/yss-project-identity\", features = [\"test-support\"] }",
    ] {
        assert!(
            workspace_manifest.contains(declaration),
            "the workspace and root package must declare {declaration}"
        );
    }

    let project_module = std::fs::read_to_string(root.join("src-tauri/src/project/mod.rs"))
        .expect("the root project module must be readable");
    for facade in [
        "pub mod identity",
        "mod project_session_id",
        "pub use yss_project_identity",
        "pub use identity::",
        "pub use project_session_id::",
    ] {
        assert!(
            !project_module.contains(facade),
            "the root project module must not restore identity facade '{facade}'"
        );
    }

    let identity =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-project-identity/src/identity.rs"))
            .expect("project identity source must be readable");
    assert!(
        identity.contains("cfg(any(test, feature = \"test-support\"))"),
        "test-only revision advancement must stay behind the explicit feature boundary"
    );
    for removed in [
        "ProjectTransactionRevision",
        "pub const fn as_uuid",
        "test transaction revision is available",
    ] {
        assert!(
            !identity.contains(removed),
            "project identity must not restore unused API '{removed}'"
        );
    }
    let session_identity = std::fs::read_to_string(
        root.join("src-tauri/crates/yss-project-identity/src/project_session_id.rs"),
    )
    .expect("project session identity source must be readable");
    assert!(
        !session_identity.contains("pub fn unknown"),
        "project session identity must not restore the zero-caller unknown sentinel"
    );
}

#[test]
fn project_registry_contract_has_one_pure_owner_without_storage_or_identity_mirrors() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-project-registry-contract/Cargo.toml",
        "src-tauri/crates/yss-project-registry-contract/src/lib.rs",
        "src-tauri/crates/yss-project-registry/Cargo.toml",
        "src-tauri/crates/yss-project-registry/src/lib.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "project registry owner must exist at {relative}"
        );
    }
    assert!(
        !root
            .join("src-tauri/src/project/project_registry_store.rs")
            .exists(),
        "the root crate must not retain a mirrored project registry storage model"
    );
    assert!(
        !root
            .join("src-tauri/src/project/project_registry.rs")
            .exists(),
        "the root crate must not retain the extracted project registry workflow"
    );

    let workspace_manifest = std::fs::read_to_string(root.join("src-tauri/Cargo.toml"))
        .expect("the Rust workspace manifest must be readable");
    for declaration in [
        "\"crates/yss-project-registry\"",
        "\"crates/yss-project-registry-contract\"",
        "yss-project-registry = { path = \"./crates/yss-project-registry\" }",
        "yss-project-registry-contract = { path = \"./crates/yss-project-registry-contract\" }",
    ] {
        assert!(
            workspace_manifest.contains(declaration),
            "the workspace and root package must declare {declaration}"
        );
    }

    let owner = std::fs::read_to_string(
        root.join("src-tauri/crates/yss-project-registry-contract/src/lib.rs"),
    )
    .expect("project registry contract owner must be readable");
    for contract in [
        "pub struct ProjectRecord",
        "pub enum ProjectRootIdentityState",
        "pub trait ProjectRegistryStore",
        "pub enum ProjectRegistryStoreError",
        "pub type ProjectRegistryStoreFuture",
        "pub id: ProjectRegistrationId",
        "deny_unknown_fields",
    ] {
        assert!(
            owner.contains(contract),
            "project registry contract must own '{contract}'"
        );
    }
    assert!(
        !owner.contains("ProjectRegistryRecord"),
        "the persistence port must consume the canonical ProjectRecord directly"
    );
    assert!(
        !owner.contains("Conflict"),
        "the registry store contract must not retain an error variant no adapter can produce"
    );

    let registry =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-project-registry/src/lib.rs"))
            .expect("project registry workflow must be readable");
    for workflow in [
        "pub struct ProjectRegistry",
        "pub enum ProjectRegistryError",
        "pub fn validate_new_project_path",
        "pub fn normalize_existing_path",
        "pub async fn scan_directory",
        "pub async fn cleanup_invalid_projects",
    ] {
        assert!(
            registry.contains(workflow),
            "project registry crate must own workflow '{workflow}'"
        );
    }
    for removed_mirror in [
        "pub struct ProjectRecord",
        "pub enum ProjectRootIdentityState",
        "ProjectRegistryRecord",
        "to_store_record",
        "from_store_record",
        "ProjectInstanceId",
    ] {
        assert!(
            !registry.contains(removed_mirror),
            "project registry workflow must not restore mirror or conflated identity '{removed_mirror}'"
        );
    }
    for misplaced_concern in [
        "crate::backend_adapters",
        "sqlx::",
        "tauri::",
        "ProjectState",
        "fail_project_remove_for_test",
        "is_registered_project_valid",
    ] {
        assert!(
            !registry.contains(misplaced_concern),
            "project registry crate must not absorb or retain '{misplaced_concern}'"
        );
    }

    for relative in [
        "src-tauri/crates/yss-project-registry/src/lib.rs",
        "src-tauri/src/application/events.rs",
        "src-tauri/src/application/project_lifecycle/mod.rs",
        "src-tauri/src/commands/command_project/registry.rs",
        "src-tauri/src/schema/application_event.rs",
        "src-tauri/src/backend_adapters/project_registry_sqlite.rs",
    ] {
        let consumer = std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("{relative} must be readable: {error}"));
        assert!(
            consumer.contains("yss_project_registry_contract::"),
            "{relative} must consume the canonical registry contract directly"
        );
    }

    let lifecycle =
        std::fs::read_to_string(root.join("src-tauri/src/application/project_lifecycle/mod.rs"))
            .expect("project lifecycle application must be readable");
    assert!(
        lifecycle.contains("#[cfg(test)]")
            && lifecycle.contains("mod tests;")
            && !lifecycle.contains("#[cfg(all(test, any()))]"),
        "project registry lifecycle tests must remain executable after the contract cutover"
    );
    assert!(
        lifecycle.contains("LifecycleRecoveryAction::RegisterDestination"),
        "a committed destination with a failed registry write must request registration recovery"
    );

    let lifecycle_transport =
        std::fs::read_to_string(root.join("src-tauri/src/schema/application_event.rs"))
            .expect("project lifecycle transport mapper must be readable");
    assert!(
        lifecycle_transport
            .contains("LifecycleRecoveryAction::RegisterDestination => \"registerDestination\"",),
        "registration recovery must preserve the existing registerDestination wire value"
    );

    let filesystem_root =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-project-filesystem/src/root.rs"))
            .expect("project filesystem root must be readable");
    assert!(
        filesystem_root.contains("yss_project_identity::ProjectRootIdentity")
            && !filesystem_root.contains("pub struct ProjectRootIdentity"),
        "the filesystem must construct the canonical root identity without owning a mirror"
    );

    let adapter = std::fs::read_to_string(
        root.join("src-tauri/src/backend_adapters/project_registry_sqlite.rs"),
    )
    .expect("project registry SQLite adapter must be readable");
    assert!(
        !adapter.contains("crate::project::"),
        "the SQLite adapter must not depend backwards on the Project workflow layer"
    );

    let project_module = std::fs::read_to_string(root.join("src-tauri/src/project/mod.rs"))
        .expect("the root project module must be readable");
    for facade in [
        "pub mod project_registry",
        "pub use project_registry",
        "pub use yss_project_registry",
        "project_registry_store",
        "pub use yss_project_registry_contract",
        "pub use yss_project_identity::ProjectRootIdentity",
    ] {
        assert!(
            !project_module.contains(facade),
            "Project must not restore registry compatibility facade '{facade}'"
        );
    }

    let policy = std::fs::read_to_string(root.join("src-tauri/src/architecture_tests/policy.rs"))
        .expect("Rust architecture policy must be readable");
    assert!(
        policy.contains("| \"yss-project-registry-contract\""),
        "project registry contract must be classified as a Pure Leaf"
    );
    assert!(
        policy.contains("|| package == \"yss-project-registry\""),
        "project registry workflow must be classified in the Project layer"
    );
    for removed_capability in [
        "project_registry_store::ProjectRegistry",
        "project::project_registry::ProjectRecord",
        "project::filesystem::root::ProjectRootIdentity",
    ] {
        assert!(
            !policy.contains(removed_capability),
            "architecture policy must not retain removed capability '{removed_capability}'"
        );
    }
}

#[test]
fn computation_settings_has_one_strict_crate_owner_without_root_or_error_mirrors() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-computation-settings/Cargo.toml",
        "src-tauri/crates/yss-computation-settings/src/lib.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "computation settings owner must exist at {relative}"
        );
    }
    assert!(
        !root
            .join("src-tauri/src/project/computation_settings.rs")
            .exists(),
        "the root project crate must not retain a computation-settings owner"
    );

    let workspace_manifest = std::fs::read_to_string(root.join("src-tauri/Cargo.toml"))
        .expect("the Rust workspace manifest must be readable");
    for declaration in [
        "\"crates/yss-computation-settings\"",
        "yss-computation-settings = { path = \"./crates/yss-computation-settings\" }",
    ] {
        assert!(
            workspace_manifest.contains(declaration),
            "the workspace and root package must declare {declaration}"
        );
    }

    let project_module = std::fs::read_to_string(root.join("src-tauri/src/project/mod.rs"))
        .expect("the root project module must be readable");
    for facade in [
        "mod computation_settings",
        "pub use computation_settings",
        "pub use yss_computation_settings",
    ] {
        assert!(
            !project_module.contains(facade),
            "the root project module must not restore settings facade '{facade}'"
        );
    }

    let owner =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-computation-settings/src/lib.rs"))
            .expect("computation settings owner must be readable");
    for contract in [
        "pub struct ProjectComputationSettings",
        "pub enum ComputationSettingsValidationError",
        "pub struct ComputationSettingsSnapshot",
        "pub struct ComputationSettingsMutationRequest",
        "pub struct ComputationSettingsMutationReceipt",
        "deny_unknown_fields",
    ] {
        assert!(
            owner.contains(contract),
            "computation settings crate must own strict contract '{contract}'"
        );
    }
    assert!(
        !owner.contains("tauri"),
        "computation settings contract must remain platform-neutral"
    );

    for relative in [
        "src-tauri/src/application/computation_settings.rs",
        "src-tauri/src/commands/command_project/settings.rs",
        "src-tauri/src/event/event_project.rs",
        "src-tauri/src/project/execution_authority.rs",
        "src-tauri/crates/yss-project-model/src/lib.rs",
        "src-tauri/crates/yss-project-manifest/src/lib.rs",
        "src-tauri/src/project/project_state.rs",
    ] {
        let consumer = std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("{relative} must be readable: {error}"));
        assert!(
            consumer.contains("yss_computation_settings"),
            "{relative} must consume the computation settings owner directly"
        );
        assert!(
            !consumer.contains("project::computation_settings"),
            "{relative} must not restore the removed root settings path"
        );
    }

    let application =
        std::fs::read_to_string(root.join("src-tauri/src/application/computation_settings.rs"))
            .expect("computation settings application adapter must be readable");
    for duplicate in ["ComputationSettingsMappingError", "fn validate("] {
        assert!(
            !application.contains(duplicate),
            "application must not restore mirrored validation logic '{duplicate}'"
        );
    }

    let manifest_owner =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-project-manifest/src/lib.rs"))
            .expect("project manifest owner must be readable");
    for validation_boundary in [
        "deserialize_valid_computation_settings",
        "settings.validate().map_err",
    ] {
        assert!(
            manifest_owner.contains(validation_boundary),
            "project manifest reads must enforce '{validation_boundary}'"
        );
    }
}

#[test]
fn project_layout_has_one_pure_crate_owner_without_domain_mirrors() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-project-layout/Cargo.toml",
        "src-tauri/crates/yss-project-layout/src/lib.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "project layout owner must exist at {relative}"
        );
    }

    let workspace_manifest = std::fs::read_to_string(root.join("src-tauri/Cargo.toml"))
        .expect("the Rust workspace manifest must be readable");
    for declaration in [
        "\"crates/yss-project-layout\"",
        "yss-project-layout = { path = \"./crates/yss-project-layout\" }",
    ] {
        assert!(
            workspace_manifest.contains(declaration),
            "the workspace and root package must declare {declaration}"
        );
    }

    let owner =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-project-layout/src/lib.rs"))
            .expect("project layout owner must be readable");
    for contract in [
        "pub const PROJECT_METADATA_FILE",
        "pub const GLOBAL_VARIABLES_FILE",
        "pub const EVENTS_DIR",
        "pub const EVENT_EXTENSION",
        "pub const FUNCTIONS_DIR",
        "pub const FUNCTION_EXTENSION",
        "pub const WORKSHEETS_DIR",
        "pub const WORKSHEET_EXTENSION",
        "pub const DATABASE_DIR",
        "pub const PROJECT_DUCKDB_FILE",
        "pub const PROJECT_CONTENT_DIRECTORIES",
        "pub fn is_project_index_input_path",
    ] {
        assert!(
            owner.contains(contract),
            "project layout crate must own canonical contract '{contract}'"
        );
    }
    for forbidden in ["std::fs", "serde", "thiserror", "tauri"] {
        assert!(
            !owner.contains(forbidden),
            "project layout must remain a dependency-free Pure Leaf without '{forbidden}'"
        );
    }

    for relative in [
        "src-tauri/crates/yss-graph-document/Cargo.toml",
        "src-tauri/crates/yss-worksheet-document/Cargo.toml",
    ] {
        let manifest = std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("{relative} must be readable: {error}"));
        assert!(
            manifest.contains("yss-project-layout = { path = \"../yss-project-layout\" }"),
            "{relative} must declare the canonical project layout dependency"
        );
    }

    for relative in [
        "src-tauri/crates/yss-project-discovery/src/lib.rs",
        "src-tauri/crates/yss-graph-document/src/resource_path.rs",
        "src-tauri/crates/yss-worksheet-document/src/lib.rs",
        "src-tauri/src/project/graph_resource_index.rs",
        "src-tauri/crates/yss-project-change/src/lib.rs",
        "src-tauri/src/project/project_io.rs",
        "src-tauri/src/project/project_lifecycle.rs",
        "src-tauri/crates/yss-project-registry/src/lib.rs",
        "src-tauri/src/project/worksheet_io.rs",
    ] {
        let consumer = std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("{relative} must be readable: {error}"));
        assert!(
            consumer.contains("yss_project_layout"),
            "{relative} must consume the canonical project layout directly"
        );
    }

    for (relative, removed_owner) in [
        (
            "src-tauri/crates/yss-project-registry/src/lib.rs",
            "pub const PROJECT_METADATA_FILE",
        ),
        (
            "src-tauri/src/project/project_io.rs",
            "pub const GLOBAL_VARIABLES_FILE",
        ),
        (
            "src-tauri/crates/yss-project-change/src/lib.rs",
            "pub fn is_relevant_project_path",
        ),
        (
            "src-tauri/crates/yss-graph-document/src/resource_path.rs",
            "const EVENTS_DIR",
        ),
        (
            "src-tauri/crates/yss-worksheet-document/src/lib.rs",
            "pub const WORKSHEETS_DIR",
        ),
    ] {
        let source = std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("{relative} must be readable: {error}"));
        assert!(
            !source.contains(removed_owner),
            "{relative} must not restore mirrored layout owner '{removed_owner}'"
        );
    }

    let project_module = std::fs::read_to_string(root.join("src-tauri/src/project/mod.rs"))
        .expect("the root project module must be readable");
    assert!(
        !project_module.contains("pub use yss_project_layout"),
        "Project must not restore a project-layout compatibility facade"
    );

    let policy = std::fs::read_to_string(root.join("src-tauri/src/architecture_tests/policy.rs"))
        .expect("Rust architecture policy must be readable");
    assert!(
        policy.contains("| \"yss-project-layout\""),
        "project layout must be classified as a Pure Leaf"
    );
}

#[test]
fn project_change_has_one_pure_owner_without_fake_watcher_files_or_root_facade() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-project-change/Cargo.toml",
        "src-tauri/crates/yss-project-change/src/lib.rs",
        "src-tauri/src/project/project_change_reconciliation.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "project change boundary must exist at {relative}"
        );
    }
    assert!(
        !root
            .join("src-tauri/src/project/project_change.rs")
            .exists(),
        "the root crate must not retain a project-change contract owner"
    );

    let workspace_manifest = std::fs::read_to_string(root.join("src-tauri/Cargo.toml"))
        .expect("the Rust workspace manifest must be readable");
    for declaration in [
        "\"crates/yss-project-change\"",
        "yss-project-change = { path = \"./crates/yss-project-change\" }",
    ] {
        assert!(
            workspace_manifest.contains(declaration),
            "the workspace and root package must declare {declaration}"
        );
    }

    let manifest =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-project-change/Cargo.toml"))
            .expect("project change manifest must be readable");
    for dependency in [
        "yss-project-identity = { path = \"../yss-project-identity\" }",
        "yss-project-layout = { path = \"../yss-project-layout\" }",
    ] {
        assert!(
            manifest.contains(dependency),
            "project change must declare its canonical dependency {dependency}"
        );
    }
    for forbidden in ["serde", "thiserror", "notify", "tauri"] {
        assert!(
            !manifest.contains(forbidden),
            "project change must remain a Pure Leaf without external concern '{forbidden}'"
        );
    }

    let owner =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-project-change/src/lib.rs"))
            .expect("project change owner must be readable");
    for contract in [
        "pub enum ProjectRelativePathError",
        "pub struct ProjectRelativePath",
        "pub enum ProjectFileChangeKind",
        "pub struct ProjectFileChange",
        "pub enum ProjectChange",
        "RescanRequired",
        "pub struct ProjectIndexInvalidation",
    ] {
        assert!(
            owner.contains(contract),
            "project change crate must own canonical contract '{contract}'"
        );
    }
    for fake_file_encoding in ["WatcherError", "PROJECT_METADATA_FILE", "watcher_error()"] {
        assert!(
            !owner.contains(fake_file_encoding),
            "source uncertainty must not be encoded as fake file fact '{fake_file_encoding}'"
        );
    }

    let project_module = std::fs::read_to_string(root.join("src-tauri/src/project/mod.rs"))
        .expect("the root project module must be readable");
    for facade in ["pub mod project_change;", "pub use yss_project_change"] {
        assert!(
            !project_module.contains(facade),
            "the root project module must not restore project-change facade '{facade}'"
        );
    }

    for relative in [
        "src-tauri/src/project/project_change_reconciliation.rs",
        "src-tauri/src/application/project_watcher.rs",
        "src-tauri/src/platform/project_file_watcher.rs",
    ] {
        let consumer = std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("{relative} must be readable: {error}"));
        assert!(
            consumer.contains("yss_project_change"),
            "{relative} must consume the project-change owner directly"
        );
    }

    let reconciliation = std::fs::read_to_string(
        root.join("src-tauri/src/project/project_change_reconciliation.rs"),
    )
    .expect("project change reconciliation must be readable");
    assert!(
        reconciliation.contains("return Ok(None)"),
        "irrelevant changes must be modeled as a no-op instead of an error"
    );
    for duplicate in [
        "pub struct ProjectRelativePath(",
        "pub struct ProjectFileChange {",
        "pub enum ProjectChange {",
        "pub struct ProjectIndexInvalidation {",
    ] {
        assert!(
            !reconciliation.contains(duplicate),
            "Project reconciliation must not mirror contract '{duplicate}'"
        );
    }

    let platform =
        std::fs::read_to_string(root.join("src-tauri/src/platform/project_file_watcher.rs"))
            .expect("project watcher platform adapter must be readable");
    for required in [
        "ProjectChange::rescan_required()",
        "AccessKind::Close(AccessMode::Write)",
        "EventKind::Access(_) => None",
        "ModifyKind::Name(_)",
    ] {
        assert!(
            platform.contains(required),
            "watcher adapter must preserve canonical behavior '{required}'"
        );
    }

    let policy = std::fs::read_to_string(root.join("src-tauri/src/architecture_tests/policy.rs"))
        .expect("Rust architecture policy must be readable");
    assert!(
        policy.contains("| \"yss-project-change\""),
        "project change must be classified as a Pure Leaf"
    );
}

#[test]
fn project_discovery_has_one_project_crate_owner_without_root_facade_or_redirect_traversal() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-project-discovery/Cargo.toml",
        "src-tauri/crates/yss-project-discovery/src/lib.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "project discovery owner must exist at {relative}"
        );
    }
    assert!(
        !root.join("src-tauri/src/project/project_scan.rs").exists(),
        "the root crate must not retain a second project-discovery owner"
    );

    let workspace_manifest = std::fs::read_to_string(root.join("src-tauri/Cargo.toml"))
        .expect("the Rust workspace manifest must be readable");
    for declaration in [
        "\"crates/yss-project-discovery\"",
        "yss-project-discovery = { path = \"./crates/yss-project-discovery\" }",
    ] {
        assert!(
            workspace_manifest.contains(declaration),
            "the workspace and root package must declare {declaration}"
        );
    }

    let manifest =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-project-discovery/Cargo.toml"))
            .expect("project discovery manifest must be readable");
    for dependency in [
        "yss-project-layout = { path = \"../yss-project-layout\" }",
        "yss-project-progress = { path = \"../yss-project-progress\" }",
    ] {
        assert!(
            manifest.contains(dependency),
            "project discovery must declare its canonical dependency {dependency}"
        );
    }

    let owner =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-project-discovery/src/lib.rs"))
            .expect("project discovery owner must be readable");
    for contract in [
        "pub const DEFAULT_PROJECT_NAME",
        "pub enum ProjectDiscoveryError",
        "pub fn normalize_project_name",
        "pub fn discover_project_metadata_files",
        "pub fn project_name_from_metadata_path",
    ] {
        assert!(
            owner.contains(contract),
            "project discovery crate must own '{contract}'"
        );
    }
    for redirect_guard in [
        "file_type.is_symlink()",
        "FILE_ATTRIBUTE_REPARSE_POINT",
        "is_redirect(root, &root_metadata.file_type())?",
        "is_redirect(&metadata_path, &metadata.file_type())?",
        "is_redirect(&path, &file_type)?",
    ] {
        assert!(
            owner.contains(redirect_guard),
            "project discovery must reject redirected directory traversal via '{redirect_guard}'"
        );
    }
    for misplaced_owner in [
        "pub struct ScanProjectsResult",
        "ProjectRegistryStore",
        "yss_project_registry_contract",
        "tauri",
        "sqlx",
    ] {
        assert!(
            !owner.contains(misplaced_owner),
            "project discovery must not absorb registry/transport concern '{misplaced_owner}'"
        );
    }

    let project_module = std::fs::read_to_string(root.join("src-tauri/src/project/mod.rs"))
        .expect("the root project module must be readable");
    for facade in ["project_scan", "pub use yss_project_discovery"] {
        assert!(
            !project_module.contains(facade),
            "the root project module must not restore discovery facade '{facade}'"
        );
    }

    for relative in [
        "src-tauri/src/project/project_lifecycle.rs",
        "src-tauri/crates/yss-project-registry/src/lib.rs",
    ] {
        let consumer = std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("{relative} must be readable: {error}"));
        assert!(
            consumer.contains("yss_project_discovery::"),
            "{relative} must consume the project discovery owner directly"
        );
        assert!(
            !consumer.contains("crate::project::project_scan"),
            "{relative} must not restore the removed root discovery path"
        );
    }

    let registry =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-project-registry/src/lib.rs"))
            .expect("project registry workflow must be readable");
    assert!(
        registry.contains("pub struct ScanProjectsResult"),
        "registration scan outcome must remain with the registry workflow"
    );

    let policy = std::fs::read_to_string(root.join("src-tauri/src/architecture_tests/policy.rs"))
        .expect("Rust architecture policy must be readable");
    assert!(
        policy.contains(
            "\"yss-project-discovery\" | \"yss-project-history\" | \"yss-project-model\""
        ) && policy.contains("layers.insert(RustLayer::Project)"),
        "project discovery must be classified as Project behavior, not as a Pure Leaf"
    );
}

#[test]
fn project_history_has_one_project_crate_owner_without_root_facade_or_ghost_graph_patch() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-project-history/Cargo.toml",
        "src-tauri/crates/yss-project-history/src/lib.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "project history owner must exist at {relative}"
        );
    }
    assert!(
        !root.join("src-tauri/src/project/history.rs").exists(),
        "the root crate must not retain a second project-history owner"
    );

    let workspace_manifest = std::fs::read_to_string(root.join("src-tauri/Cargo.toml"))
        .expect("the Rust workspace manifest must be readable");
    for declaration in [
        "\"crates/yss-project-history\"",
        "yss-project-history = { path = \"./crates/yss-project-history\" }",
    ] {
        assert!(
            workspace_manifest.contains(declaration),
            "the workspace and root package must declare {declaration}"
        );
    }
    assert!(
        !workspace_manifest
            .contains("yss-project-history = { path = \"./crates/yss-project-history\", features"),
        "project history must not restore a test-only ghost API through feature unification"
    );

    let manifest =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-project-history/Cargo.toml"))
            .expect("project history manifest must be readable");
    assert!(
        !manifest.contains("yss-graph-document-edit") && !manifest.contains("test-support"),
        "project history must not depend on Graph editing solely to preserve a dead test API"
    );

    let owner =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-project-history/src/lib.rs"))
            .expect("project history owner must be readable");
    for contract in [
        "pub struct MutationRequest",
        "pub enum ResourceKey",
        "pub struct ResourcePatch",
        "pub struct ProjectHistoryTransaction",
        "pub struct ProjectDocumentState",
        "pub enum HistoryError",
        "pub struct ProjectHistory",
    ] {
        assert!(
            owner.contains(contract),
            "project history crate must own canonical contract or behavior '{contract}'"
        );
    }
    for forbidden in [
        "std::fs",
        "tauri",
        "crate::project",
        "cfg(all(test, any()))",
        "GraphDocumentPatch",
        "ResourceDocumentPatch::Graph",
        "pub fn graph(",
    ] {
        assert!(
            !owner.contains(forbidden),
            "project history must not restore deprecated or adapter coupling '{forbidden}'"
        );
    }

    let project_module = std::fs::read_to_string(root.join("src-tauri/src/project/mod.rs"))
        .expect("the root project module must be readable");
    for facade in [
        "pub mod history;",
        "pub use history::",
        "pub use yss_project_history",
    ] {
        assert!(
            !project_module.contains(facade),
            "Project must not restore project-history compatibility facade '{facade}'"
        );
    }

    for relative in [
        "src-tauri/src/application/resource_mutation.rs",
        "src-tauri/src/project/history_hydration.rs",
        "src-tauri/src/project/project_state.rs",
        "src-tauri/src/schema/application_event.rs",
    ] {
        let consumer = std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("{relative} must be readable: {error}"));
        assert!(
            consumer.contains("yss_project_history"),
            "{relative} must consume the canonical project history owner directly"
        );
        assert!(
            !consumer.contains("crate::project::history::"),
            "{relative} must not restore the removed root history path"
        );
    }

    let policy = std::fs::read_to_string(root.join("src-tauri/src/architecture_tests/policy.rs"))
        .expect("Rust architecture policy must be readable");
    assert!(
        policy.contains(
            "\"yss-project-discovery\" | \"yss-project-history\" | \"yss-project-model\""
        ) && policy.contains("layers.insert(RustLayer::Project)"),
        "project history must be classified as Project behavior, not as a Pure Leaf"
    );
}

#[test]
fn project_model_has_one_clock_free_owner_without_root_facade_or_duplicate_graph_kind() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-project-model/Cargo.toml",
        "src-tauri/crates/yss-project-model/src/lib.rs",
        "src-tauri/crates/yss-project-model/src/patch.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "project model owner must exist at {relative}"
        );
    }
    for removed_owner in [
        "src-tauri/src/project/project_data.rs",
        "src-tauri/src/project/project_metadata.rs",
        "src-tauri/src/project/resource_patch.rs",
    ] {
        assert!(
            !root.join(removed_owner).exists(),
            "the root crate must not retain project-model owner {removed_owner}"
        );
    }

    let workspace_manifest = std::fs::read_to_string(root.join("src-tauri/Cargo.toml"))
        .expect("the Rust workspace manifest must be readable");
    for declaration in [
        "\"crates/yss-project-model\"",
        "yss-project-model = { path = \"./crates/yss-project-model\" }",
    ] {
        assert!(
            workspace_manifest.contains(declaration),
            "the workspace and root package must declare {declaration}"
        );
    }

    let manifest =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-project-model/Cargo.toml"))
            .expect("project model manifest must be readable");
    for dependency in [
        "serde.workspace = true",
        "yss-computation-settings = { path = \"../yss-computation-settings\" }",
        "yss-database-contract = { path = \"../yss-database-contract\" }",
        "yss-graph-document = { path = \"../yss-graph-document\" }",
        "yss-project-history = { path = \"../yss-project-history\" }",
        "yss-project-identity = { path = \"../yss-project-identity\" }",
        "yss-variable-contract = { path = \"../yss-variable-contract\" }",
        "yss-worksheet-document = { path = \"../yss-worksheet-document\" }",
    ] {
        assert!(
            manifest.contains(dependency),
            "project model must declare canonical dependency {dependency}"
        );
    }
    for forbidden in ["chrono", "tauri", "sqlx", "polars"] {
        assert!(
            !manifest.contains(forbidden),
            "project model must not absorb runtime dependency '{forbidden}'"
        );
    }

    let owner = std::fs::read_to_string(root.join("src-tauri/crates/yss-project-model/src/lib.rs"))
        .expect("project model owner must be readable");
    for contract in [
        "pub struct GraphResourceDocument",
        "pub struct ProjectMetadata",
        "pub struct ProjectData",
        "pub fn new() -> Self",
        "function: matches!(kind, GraphResourceKind::Function)",
        "project_name: \"未命名项目\".to_owned()",
        "export_time: String::new()",
        "pub computation_settings: ProjectComputationSettings",
        "pub graphs: HashMap<GraphResourcePath, GraphResourceDocument>",
        "pub worksheets: HashMap<WorksheetResourcePath, WorksheetDocument>",
    ] {
        assert!(
            owner.contains(contract),
            "project model crate must own invariant '{contract}'"
        );
    }
    assert!(
        owner.contains("#[derive(Debug, Default, Clone)]\npub struct ProjectData")
            && owner.contains("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct ProjectMetadata"),
        "the in-memory aggregate and metadata must not become implicit wire contracts"
    );
    for removed_or_misplaced in [
        "pub fn info",
        "pub fn to_json",
        "pub fn from_json",
        "pub fn update_metadata",
        "chrono::",
        "ProjectError",
        "std::fs",
        "tauri::",
    ] {
        assert!(
            !owner.contains(removed_or_misplaced),
            "project model must not restore dead or runtime concern '{removed_or_misplaced}'"
        );
    }

    let graph_kind_owner = std::fs::read_to_string(
        root.join("src-tauri/crates/yss-graph-document/src/resource_path.rs"),
    )
    .expect("graph resource kind owner must be readable");
    assert!(
        graph_kind_owner
            .contains("#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]",)
            && graph_kind_owner
                .contains("#[serde(rename_all = \"lowercase\")]\npub enum GraphResourceKind"),
        "the canonical graph resource kind must own its stable lower-case wire"
    );

    let project_module = std::fs::read_to_string(root.join("src-tauri/src/project/mod.rs"))
        .expect("the root project module must be readable");
    for facade in [
        "mod project_data",
        "mod project_metadata",
        "pub use yss_project_model",
        "pub use project_data",
        "pub use project_metadata",
    ] {
        assert!(
            !project_module.contains(facade),
            "the root project module must not restore compatibility facade '{facade}'"
        );
    }

    let project_io = std::fs::read_to_string(root.join("src-tauri/src/project/project_io.rs"))
        .expect("Project IO must be readable");
    for duplicate in ["pub enum GraphDocumentKind", "pub enum GraphResourceKind"] {
        assert!(
            !project_io.contains(duplicate),
            "Project IO must not restore duplicate graph kind '{duplicate}'"
        );
    }

    for relative in [
        "src-tauri/src/application/execution/run_graph.rs",
        "src-tauri/src/application/resource_mutation.rs",
        "src-tauri/src/project/history_hydration.rs",
        "src-tauri/src/project/project_io.rs",
        "src-tauri/src/project/project_lifecycle.rs",
        "src-tauri/src/project/project_state.rs",
    ] {
        let consumer = std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("{relative} must be readable: {error}"));
        assert!(
            consumer.contains("yss_project_model"),
            "{relative} must consume the canonical project model directly"
        );
        for removed_path in [
            "crate::project::ProjectData",
            "crate::project::GraphResourceDocument",
        ] {
            assert!(
                !consumer.contains(removed_path),
                "{relative} must not restore removed model path '{removed_path}'"
            );
        }
    }

    let lifecycle =
        std::fs::read_to_string(root.join("src-tauri/src/project/project_lifecycle.rs"))
            .expect("project lifecycle must be readable");
    assert!(
        lifecycle.contains("data.metadata.export_time = current_export_time();")
            && lifecycle.contains("chrono::Utc::now().to_rfc3339()"),
        "lifecycle must own the explicit export-time clock read"
    );

    let policy = std::fs::read_to_string(root.join("src-tauri/src/architecture_tests/policy.rs"))
        .expect("Rust architecture policy must be readable");
    assert!(
        policy.contains(
            "\"yss-project-discovery\" | \"yss-project-history\" | \"yss-project-model\"",
        ) && policy.contains("layers.insert(RustLayer::Project)"),
        "project model must be classified as Project behavior, not as a Pure Leaf"
    );
    assert!(
        !policy.contains("project_io::GraphResourceKind"),
        "architecture capabilities must not preserve the deleted graph-kind facade"
    );
}

#[test]
fn project_data_patch_has_one_model_owner_without_history_name_collision() {
    let root = repository_root();
    let patch_owner =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-project-model/src/patch.rs"))
            .expect("project data patch owner must be readable");
    for contract in [
        "pub enum ProjectDataPatch",
        "use yss_project_identity::ResourceRevision",
        "InsertGraph",
        "MoveGraph",
        "moved_before: Box<GraphResourceDocument>",
        "PatchVariables",
        "UpsertWorksheet",
    ] {
        assert!(
            patch_owner.contains(contract),
            "project model must own aggregate patch contract '{contract}'"
        );
    }
    for misplaced in [
        "pub enum ResourceDocumentPatch",
        "crate::project",
        "ProjectState",
        "std::fs",
    ] {
        assert!(
            !patch_owner.contains(misplaced),
            "project data patch must not absorb '{misplaced}'"
        );
    }

    let model_root =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-project-model/src/lib.rs"))
            .expect("project model root must be readable");
    assert!(
        model_root.contains("mod patch;")
            && model_root.contains("pub use patch::ProjectDataPatch;"),
        "project model must expose its aggregate patch directly"
    );

    let history_owner =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-project-history/src/lib.rs"))
            .expect("project history owner must be readable");
    assert!(
        history_owner.contains("pub enum ResourceDocumentPatch"),
        "persisted history payloads must retain their distinct canonical owner"
    );

    let project_module = std::fs::read_to_string(root.join("src-tauri/src/project/mod.rs"))
        .expect("root project module must be readable");
    for facade in [
        "mod resource_patch",
        "pub mod resource_patch",
        "pub use resource_patch",
    ] {
        assert!(
            !project_module.contains(facade),
            "root project module must not restore aggregate patch facade '{facade}'"
        );
    }
    assert!(
        !root
            .join("src-tauri/src/project/resource_patch.rs")
            .exists(),
        "the deleted root aggregate patch owner must stay absent"
    );

    for relative in [
        "src-tauri/src/project/project_writers.rs",
        "src-tauri/src/project/project_writers/worksheets.rs",
        "src-tauri/src/project/project_state/graph_rename.rs",
        "src-tauri/src/project/project_state/history_moves.rs",
        "src-tauri/src/project/project_state/resource_history.rs",
        "src-tauri/src/project/project_state/resource_patch.rs",
    ] {
        let consumer = std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("{relative} must be readable: {error}"));
        assert!(
            consumer.contains("ProjectDataPatch"),
            "{relative} must use the unambiguous aggregate patch name"
        );
        assert!(
            !consumer.contains("crate::project::resource_patch"),
            "{relative} must consume the project model owner directly"
        );
    }
}

#[test]
fn project_operation_has_one_stateful_owner_without_root_ledger_or_private_epoch() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-project-operation/Cargo.toml",
        "src-tauri/crates/yss-project-operation/src/lib.rs",
        "src-tauri/src/project/project_operation_admission.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "project operation boundary must exist at {relative}"
        );
    }
    assert!(
        !root
            .join("src-tauri/src/project/resource_mutations/operation_ledger.rs")
            .exists(),
        "the root crate must not retain the project operation ledger owner"
    );

    let workspace_manifest = std::fs::read_to_string(root.join("src-tauri/Cargo.toml"))
        .expect("the Rust workspace manifest must be readable");
    for declaration in [
        "\"crates/yss-project-operation\"",
        "yss-project-operation = { path = \"./crates/yss-project-operation\" }",
    ] {
        assert!(
            workspace_manifest.contains(declaration),
            "the workspace and root package must declare {declaration}"
        );
    }

    let manifest =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-project-operation/Cargo.toml"))
            .expect("project operation manifest must be readable");
    for dependency in [
        "thiserror.workspace = true",
        "yss-project-identity = { path = \"../yss-project-identity\" }",
    ] {
        assert!(
            manifest.contains(dependency),
            "project operation must declare canonical dependency {dependency}"
        );
    }
    for forbidden in ["uuid", "tauri", "sqlx", "polars"] {
        assert!(
            !manifest.contains(forbidden),
            "project operation must not absorb runtime concern '{forbidden}'"
        );
    }

    let owner =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-project-operation/src/lib.rs"))
            .expect("project operation owner must be readable");
    for invariant in [
        "pub struct ProjectOperationLedger",
        "pub struct ProjectOperationReservation",
        "pub enum ProjectOperationAdmissionError",
        "project_session_id: ProjectSessionId",
        "pub fn reset_for_project(",
        "in_flight: HashSet<OperationId>",
        "completed: HashSet<OperationId>",
        "impl Drop for ProjectOperationReservation",
        "pub fn complete(mut self)",
    ] {
        assert!(
            owner.contains(invariant),
            "project operation crate must own state-machine invariant '{invariant}'"
        );
    }
    for misplaced in [
        "session_epoch",
        "uuid::",
        "ProjectState",
        "ProjectFilesystemError",
        "crate::project",
    ] {
        assert!(
            !owner.contains(misplaced),
            "project operation must not restore duplicate or root concern '{misplaced}'"
        );
    }

    let bridge =
        std::fs::read_to_string(root.join("src-tauri/src/project/project_operation_admission.rs"))
            .expect("project operation admission bridge must be readable");
    for behavior in [
        "self.mutation_publication.lock().unwrap()",
        "ProjectOperationLedger::reserve(",
        "ProjectOperationAdmissionError::StaleProject { .. }",
        "ProjectFilesystemError::StaleProjectLifecycle { message }",
        "ProjectOperationAdmissionError::DuplicateOperation { .. }",
        "ProjectFilesystemError::DuplicateOperation { message }",
    ] {
        assert!(
            bridge.contains(behavior),
            "root admission bridge must retain behavior '{behavior}'"
        );
    }

    let resource_mutations =
        std::fs::read_to_string(root.join("src-tauri/src/project/resource_mutations.rs"))
            .expect("resource mutation module must be readable");
    for facade in [
        "mod operation_ledger",
        "ResourceOperationLedger",
        "ResourceOperationReservation",
    ] {
        assert!(
            !resource_mutations.contains(facade),
            "root resource mutations must not restore ledger facade '{facade}'"
        );
    }

    let project_state =
        std::fs::read_to_string(root.join("src-tauri/src/project/project_state/state.rs"))
            .expect("project state must be readable");
    assert!(
        project_state.contains("Arc<Mutex<yss_project_operation::ProjectOperationLedger>>"),
        "ProjectState must hold the extracted operation owner directly"
    );
    let activation =
        std::fs::read_to_string(root.join("src-tauri/src/project/project_state/activation.rs"))
            .expect("project activation must be readable");
    assert!(
        activation.contains("next_identity.project_session_id.clone()"),
        "activation must reset operation state with the canonical project session identity"
    );

    let policy = std::fs::read_to_string(root.join("src-tauri/src/architecture_tests/policy.rs"))
        .expect("Rust architecture policy must be readable");
    assert!(
        policy.contains("package == \"yss-project-operation\"")
            && policy.contains("layers.insert(RustLayer::Project)"),
        "project operation must be classified as stateful Project behavior"
    );
}

#[test]
fn project_filesystem_has_one_stateful_owner_without_root_facade_or_session_cycle() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-project-filesystem/Cargo.toml",
        "src-tauri/crates/yss-project-filesystem/src/lib.rs",
        "src-tauri/crates/yss-project-filesystem/src/coordinator.rs",
        "src-tauri/crates/yss-project-filesystem/src/error.rs",
        "src-tauri/crates/yss-project-filesystem/src/recovery.rs",
        "src-tauri/crates/yss-project-filesystem/src/root.rs",
        "src-tauri/crates/yss-project-filesystem/src/transaction.rs",
        "src-tauri/src/application/project_failure.rs",
        "src-tauri/src/commands/project_failure.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "project filesystem boundary must exist at {relative}"
        );
    }
    for relative in [
        "src-tauri/src/project/filesystem/mod.rs",
        "src-tauri/src/project/filesystem/coordinator.rs",
        "src-tauri/src/project/filesystem/root.rs",
        "src-tauri/src/project/filesystem/transaction.rs",
    ] {
        assert!(
            !root.join(relative).exists(),
            "the root crate must not retain filesystem owner {relative}"
        );
    }

    let workspace_manifest = std::fs::read_to_string(root.join("src-tauri/Cargo.toml"))
        .expect("the Rust workspace manifest must be readable");
    for declaration in [
        "\"crates/yss-project-filesystem\"",
        "yss-project-filesystem = { path = \"./crates/yss-project-filesystem\" }",
        "yss-project-filesystem = { path = \"./crates/yss-project-filesystem\", features = [\"test-support\"] }",
    ] {
        assert!(
            workspace_manifest.contains(declaration),
            "the workspace and root package must declare {declaration}"
        );
    }

    let manifest =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-project-filesystem/Cargo.toml"))
            .expect("project filesystem manifest must be readable");
    for dependency in [
        "serde_json.workspace = true",
        "thiserror.workspace = true",
        "unicode-casefold.workspace = true",
        "unicode-normalization.workspace = true",
        "uuid.workspace = true",
        "yss-project-identity = { path = \"../yss-project-identity\" }",
        "yss-project-layout = { path = \"../yss-project-layout\" }",
    ] {
        assert!(
            manifest.contains(dependency),
            "project filesystem must declare canonical dependency {dependency}"
        );
    }
    for forbidden in ["tauri", "sqlx", "polars", "yssbi"] {
        assert!(
            !manifest.contains(forbidden),
            "project filesystem must not absorb runtime concern '{forbidden}'"
        );
    }

    let owner =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-project-filesystem/src/lib.rs"))
            .expect("project filesystem owner must be readable");
    for invariant in [
        "ProjectFilesystemCoordinator",
        "ProjectRootLifecycleGuard",
        "ProjectFilesystemTransaction",
        "ProjectFilesystemTransactionContext",
        "ProjectRecoveryMarker",
        "NormalizedProjectRoot",
    ] {
        assert!(
            owner.contains(invariant),
            "project filesystem must own {invariant}"
        );
    }

    let transaction = std::fs::read_to_string(
        root.join("src-tauri/crates/yss-project-filesystem/src/transaction.rs"),
    )
    .expect("project filesystem transaction must be readable");
    assert!(
        transaction.contains("pub struct ProjectFilesystemTransactionContext")
            && transaction.contains("pub root: NormalizedProjectRoot")
            && !transaction.contains("ProjectSession"),
        "filesystem transactions must consume minimal filesystem facts, not ProjectSession"
    );
    assert!(
        !transaction.contains("cfg(all(test, any()))"),
        "project filesystem tests must remain executable"
    );

    let project_module = std::fs::read_to_string(root.join("src-tauri/src/project/mod.rs"))
        .expect("root project module must be readable");
    assert!(
        !project_module.contains("mod filesystem")
            && !project_module.contains("pub use yss_project_filesystem"),
        "the root Project layer must consume the crate directly without a compatibility facade"
    );
    let project_error =
        std::fs::read_to_string(root.join("src-tauri/src/project/project_error.rs"))
            .expect("root project error module must be readable");
    assert!(
        !project_error.contains("enum ProjectFilesystemError"),
        "ProjectFilesystemError must have one canonical owner"
    );
    let session = std::fs::read_to_string(root.join("src-tauri/src/project/project_session.rs"))
        .expect("project session must be readable");
    assert!(
        session.contains("ProjectFilesystemTransactionContext")
            && session.contains("fn filesystem_context(&self)"),
        "Project must adapt its rich transaction context into minimal filesystem facts"
    );

    let application_failure =
        std::fs::read_to_string(root.join("src-tauri/src/application/project_failure.rs"))
            .expect("application project failure view must be readable");
    assert!(
        application_failure.contains("ProjectFilesystemError")
            && application_failure.contains("ApplicationProjectFailure"),
        "Application must own the explicit Project failure view"
    );
    let command_failure =
        std::fs::read_to_string(root.join("src-tauri/src/commands/project_failure.rs"))
            .expect("command project failure adapter must be readable");
    let command_failure_production = command_failure
        .split_once("#[cfg(test)]")
        .map_or(command_failure.as_str(), |(production, _)| production);
    assert!(
        command_failure_production.contains("ApplicationProjectFailure")
            && !command_failure_production.contains("use yss_project_filesystem"),
        "Commands must map the Application failure view without importing Project"
    );
    let transport_error = std::fs::read_to_string(root.join("src-tauri/src/error/mod.rs"))
        .expect("transport error module must be readable");
    assert!(
        !transport_error.contains("yss_project_filesystem")
            && !transport_error.contains("ApplicationProjectFailure"),
        "the generic Transport error must remain independent of Project and Application"
    );

    let policy = std::fs::read_to_string(root.join("src-tauri/src/architecture_tests/policy.rs"))
        .expect("Rust architecture policy must be readable");
    assert!(
        policy.contains("package == \"yss-project-filesystem\"")
            && policy.contains("layers.insert(RustLayer::Project)"),
        "project filesystem must be classified as stateful Project behavior"
    );
}

#[test]
fn resource_lifecycle_has_one_stateful_owner_without_root_facade_or_disabled_tests() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-resource-lifecycle/Cargo.toml",
        "src-tauri/crates/yss-resource-lifecycle/src/lib.rs",
        "src-tauri/src/project/resource_lifecycle_operation.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "resource lifecycle boundary must exist at {relative}"
        );
    }
    assert!(
        !root
            .join("src-tauri/src/project/resource_lifecycle.rs")
            .exists(),
        "the root crate must not retain the resource lifecycle state-machine owner"
    );

    let workspace_manifest = std::fs::read_to_string(root.join("src-tauri/Cargo.toml"))
        .expect("the Rust workspace manifest must be readable");
    for declaration in [
        "\"crates/yss-resource-lifecycle\"",
        "yss-resource-lifecycle = { path = \"./crates/yss-resource-lifecycle\" }",
    ] {
        assert!(
            workspace_manifest.contains(declaration),
            "the workspace and root package must declare {declaration}"
        );
    }

    let manifest =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-resource-lifecycle/Cargo.toml"))
            .expect("resource lifecycle manifest must be readable");
    for dependency in [
        "thiserror.workspace = true",
        "yss-graph-document = { path = \"../yss-graph-document\" }",
        "yss-project-identity = { path = \"../yss-project-identity\" }",
        "yss-worksheet-document = { path = \"../yss-worksheet-document\" }",
    ] {
        assert!(
            manifest.contains(dependency),
            "resource lifecycle must declare canonical dependency {dependency}"
        );
    }
    for forbidden in ["uuid", "tauri", "sqlx", "polars"] {
        assert!(
            !manifest.contains(forbidden),
            "resource lifecycle must not absorb runtime concern '{forbidden}'"
        );
    }

    let owner =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-resource-lifecycle/src/lib.rs"))
            .expect("resource lifecycle owner must be readable");
    for invariant in [
        "pub enum LifecycleResourcePath",
        "pub enum ResourceLifecycleIntent",
        "pub struct ResourceLifecycleOwner",
        "pub struct ResourceLifecycleRegistry",
        "pub struct ResourceLifecycleBoundary",
        "pub struct ResourceLifecycleState",
        "pub struct ResourceLifecycleGuard",
        "pub enum ResourceLifecycleError",
        "state: Arc<Mutex<ResourceLifecycleState>>",
        "predecessor: Option<u64>",
        "ResourceLifecycleRegistrationState::Abandoned",
        "impl Drop for ResourceLifecycleGuard",
        "project_instance_id: &ProjectInstanceId",
        "pub fn boundary_recovering(",
        "pub fn take_state(",
        "pub fn clear_poison(",
    ] {
        assert!(
            owner.contains(invariant),
            "resource lifecycle crate must own state-machine invariant '{invariant}'"
        );
    }
    assert_eq!(
        owner.matches("#[test]").count(),
        17,
        "all formerly disabled resource lifecycle state-machine tests must remain active"
    );
    for misplaced in [
        "ProjectSession",
        "ProjectFilesystemError",
        "crate::project",
        "uuid::",
        "#[cfg(all(test, any()))]",
        "unreachable!",
        "fn graph_path(",
    ] {
        assert!(
            !owner.contains(misplaced),
            "resource lifecycle must not restore root, disabled, or dead concern '{misplaced}'"
        );
    }

    let project_module = std::fs::read_to_string(root.join("src-tauri/src/project/mod.rs"))
        .expect("project module must be readable");
    for facade in [
        "pub mod resource_lifecycle",
        "pub use resource_lifecycle",
        "pub use yss_resource_lifecycle",
    ] {
        assert!(
            !project_module.contains(facade),
            "root Project must not restore resource lifecycle facade '{facade}'"
        );
    }

    let bridge =
        std::fs::read_to_string(root.join("src-tauri/src/project/resource_lifecycle_operation.rs"))
            .expect("resource lifecycle operation bridge must be readable");
    for behavior in [
        "pub(crate) struct ResourceLifecycleOperation",
        "pub(crate) session: ProjectSession",
        "pub(crate) struct ResourceRenameOwnershipLease",
        "ProjectFilesystemError::from",
        "ResourceLifecycleIntent::Unload",
        "load_rejects_owned_document_after_project_replacement",
        "ProjectFilesystemError::StaleProjectLifecycle { .. }",
    ] {
        assert!(
            bridge.contains(behavior),
            "root bridge must retain cross-state behavior '{behavior}'"
        );
    }

    let project_error =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-project-filesystem/src/error.rs"))
            .expect("project filesystem error owner must be readable");
    for mapping in [
        "impl From<yss_resource_lifecycle::ResourceLifecycleError>",
        "Self::FilesystemTransactionBusy { message }",
        "Self::StaleResourceLifecycle { message }",
    ] {
        assert!(
            project_error.contains(mapping),
            "project filesystem error owner must retain mapping '{mapping}'"
        );
    }

    let project_state =
        std::fs::read_to_string(root.join("src-tauri/src/project/project_state.rs"))
            .expect("project state module must be readable");
    assert!(
        project_state.contains("use yss_resource_lifecycle::ResourceLifecycleRegistry;")
            && !project_state.contains("resource_lifecycle_entry_count")
            && !project_state.contains("activation_publication_guards_are_available_for_test"),
        "ProjectState must consume the crate directly without dead lifecycle probes"
    );

    let graph_lifecycle = std::fs::read_to_string(
        root.join("src-tauri/src/project/project_state/graph_lifecycle_application.rs"),
    )
    .expect("graph lifecycle application must be readable");
    assert!(
        graph_lifecycle.contains("self.run_graph_load_after_read_test_hook();")
            && graph_lifecycle.contains("self.validate_resource_lifecycle_operation(&operation)?;"),
        "graph load must retain an exercised replacement seam before lifecycle revalidation"
    );

    let activation =
        std::fs::read_to_string(root.join("src-tauri/src/project/project_state/activation.rs"))
            .expect("project activation must be readable");
    for behavior in [
        "_lifecycle: yss_resource_lifecycle::ResourceLifecycleState",
        "self.resource_lifecycle.boundary_recovering()",
        "_lifecycle: lifecycle.take_state()",
        "self.resource_lifecycle.clear_poison()",
    ] {
        assert!(
            activation.contains(behavior),
            "activation must preserve lifecycle publication behavior '{behavior}'"
        );
    }
    let activation_commit = activation
        .split_once("let mut garbage = None;")
        .expect("activation commit boundary must remain explicit")
        .1;
    let publication_lock = activation_commit
        .find("self.mutation_publication.lock()")
        .expect("activation must acquire publication authority");
    let operation_lock = activation_commit
        .find("self.resource_operations.lock()")
        .expect("activation must acquire operation authority");
    let path_lock = activation_commit
        .find("self.project_path.write()")
        .expect("activation must acquire project path authority");
    let lifecycle_lock = activation_commit
        .find("self.resource_lifecycle.boundary_recovering()")
        .expect("activation must acquire resource lifecycle authority");
    let data_lock = activation_commit
        .find("self.project_data.write()")
        .expect("activation must acquire project data authority");
    assert!(
        publication_lock < operation_lock
            && operation_lock < path_lock
            && path_lock < lifecycle_lock
            && lifecycle_lock < data_lock,
        "resource lifecycle extraction must preserve activation lock order"
    );

    for (relative, direct_dependency) in [
        (
            "src-tauri/src/project/project_state/graph_lifecycle_application.rs",
            "use yss_resource_lifecycle::{LifecycleResourcePath, ResourceLifecycleIntent};",
        ),
        (
            "src-tauri/src/project/project_state/graph_rename.rs",
            "yss_resource_lifecycle::LifecycleResourcePath::Graph",
        ),
        (
            "src-tauri/src/project/project_writers/worksheets.rs",
            "yss_resource_lifecycle::LifecycleResourcePath::Worksheet",
        ),
    ] {
        let consumer = std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("{relative} must be readable: {error}"));
        assert!(
            consumer.contains(direct_dependency),
            "{relative} must consume the resource lifecycle owner directly"
        );
        assert!(
            !consumer.contains("crate::project::LifecycleResourcePath"),
            "{relative} must not consume a root lifecycle facade"
        );
    }

    let policy = std::fs::read_to_string(root.join("src-tauri/src/architecture_tests/policy.rs"))
        .expect("Rust architecture policy must be readable");
    assert!(
        policy.contains("package == \"yss-resource-lifecycle\"")
            && policy.contains("layers.insert(RustLayer::Project)"),
        "resource lifecycle must be classified as stateful Project behavior"
    );
}

#[test]
fn function_editor_projection_has_one_project_owner_without_root_or_transport_mirror() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-function-editor-projection/Cargo.toml",
        "src-tauri/crates/yss-function-editor-projection/src/lib.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "function editor projection owner must exist at {relative}"
        );
    }
    assert!(
        !root
            .join("src-tauri/src/project/function_editor_projection.rs")
            .exists(),
        "Project must not retain the old function editor projection owner"
    );

    let workspace_manifest = std::fs::read_to_string(root.join("src-tauri/Cargo.toml"))
        .expect("the Rust workspace manifest must be readable");
    for declaration in [
        "\"crates/yss-function-editor-projection\"",
        "yss-function-editor-projection = { path = \"./crates/yss-function-editor-projection\" }",
    ] {
        assert!(
            workspace_manifest.contains(declaration),
            "the workspace and root package must declare {declaration}"
        );
    }

    let manifest = std::fs::read_to_string(
        root.join("src-tauri/crates/yss-function-editor-projection/Cargo.toml"),
    )
    .expect("function editor projection manifest must be readable");
    for dependency in [
        "serde.workspace = true",
        "thiserror.workspace = true",
        "yss-data-contract = { path = \"../yss-data-contract\" }",
        "yss-project-history = { path = \"../yss-project-history\" }",
        "yss-project-identity = { path = \"../yss-project-identity\" }",
    ] {
        assert!(
            manifest.contains(dependency),
            "function editor projection must declare canonical dependency {dependency}"
        );
    }
    for forbidden in ["chrono", "tauri", "sqlx", "polars"] {
        assert!(
            !manifest.contains(forbidden),
            "function editor projection must not absorb runtime dependency '{forbidden}'"
        );
    }

    let owner = std::fs::read_to_string(
        root.join("src-tauri/crates/yss-function-editor-projection/src/lib.rs"),
    )
    .expect("function editor projection owner must be readable");
    for invariant in [
        "pub struct FunctionEditorPin",
        "pub struct FunctionEditorProjection",
        "pub function_revision: ResourceRevision",
        "impl TryFrom<&FunctionDocument> for FunctionEditorProjection",
        "pub fn parse_function_data_type",
        "#[serde(rename_all = \"camelCase\", deny_unknown_fields)]",
    ] {
        assert!(
            owner.contains(invariant),
            "function editor projection must own invariant '{invariant}'"
        );
    }
    for removed_or_weak_shape in [
        "pub function_revision: u64",
        "pub fn build_function_editor_projection",
        "pub fn resolve_function_data_type",
        "tauri::",
        "std::fs",
    ] {
        assert!(
            !owner.contains(removed_or_weak_shape),
            "function editor projection must not restore '{removed_or_weak_shape}'"
        );
    }

    let project_module = std::fs::read_to_string(root.join("src-tauri/src/project/mod.rs"))
        .expect("the root project module must be readable");
    for facade in [
        "mod function_editor_projection",
        "pub use function_editor_projection",
        "pub use yss_function_editor_projection",
    ] {
        assert!(
            !project_module.contains(facade),
            "the root project module must not restore facade '{facade}'"
        );
    }

    for relative in [
        "src-tauri/src/project/project_io.rs",
        "src-tauri/src/project/project_reads.rs",
        "src-tauri/src/application/resource_mutation.rs",
    ] {
        let consumer = std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("{relative} must be readable: {error}"));
        assert!(
            consumer.contains("yss_function_editor_projection::FunctionEditorProjection")
                && consumer.contains("FunctionEditorProjection::try_from"),
            "{relative} must project the canonical FunctionDocument directly"
        );
        for duplicate_mapping in [
            "build_function_editor_projection",
            "function.signature.parameters.iter().map",
            "crate::project::FunctionEditorProjection",
        ] {
            assert!(
                !consumer.contains(duplicate_mapping),
                "{relative} must not restore duplicate mapping '{duplicate_mapping}'"
            );
        }
    }

    let catalog_query =
        std::fs::read_to_string(root.join("src-tauri/src/application/catalog_query.rs"))
            .expect("catalog query must be readable");
    assert!(
        catalog_query.contains("yss_function_editor_projection::parse_function_data_type")
            && !catalog_query.contains("resolve_function_data_type"),
        "catalog mapping must reuse the canonical function data-type parser"
    );

    let transport_types =
        std::fs::read_to_string(root.join("src-tauri/src/schema/editor_projection_types.rs"))
            .expect("editor projection transport types must be readable");
    for duplicate in ["FunctionEditorPinDto", "FunctionEditorProjectionDto"] {
        assert!(
            !transport_types.contains(duplicate),
            "Transport must not restore duplicate projection DTO '{duplicate}'"
        );
    }
    let application_event =
        std::fs::read_to_string(root.join("src-tauri/src/schema/application_event.rs"))
            .expect("application event transport must be readable");
    assert!(
        application_event
            .contains("Option<yss_function_editor_projection::FunctionEditorProjection>")
            && application_event.contains(".function_editor_projection\n                .clone()"),
        "event transport must reuse the canonical projection wire without field copying"
    );

    let policy = std::fs::read_to_string(root.join("src-tauri/src/architecture_tests/policy.rs"))
        .expect("Rust architecture policy must be readable");
    assert!(
        policy.contains("package == \"yss-function-editor-projection\"")
            && policy.contains("layers.insert(RustLayer::Project)"),
        "function editor projection must remain Project behavior"
    );
    assert!(
        policy.contains("\"yss_function_editor_projection::FunctionEditorProjection\"")
            && !policy.contains(
                "yssbi_lib::project::function_editor_projection::FunctionEditorProjection"
            ),
        "transport capability must point only at the canonical crate owner"
    );
}

#[test]
fn project_manifest_has_one_strict_pure_owner_without_root_wire_or_mutation_seams() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-project-manifest/Cargo.toml",
        "src-tauri/crates/yss-project-manifest/src/lib.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "project manifest owner must exist at {relative}"
        );
    }

    let workspace_manifest = std::fs::read_to_string(root.join("src-tauri/Cargo.toml"))
        .expect("the Rust workspace manifest must be readable");
    for declaration in [
        "\"crates/yss-project-manifest\"",
        "yss-project-manifest = { path = \"./crates/yss-project-manifest\" }",
    ] {
        assert!(
            workspace_manifest.contains(declaration),
            "the workspace and root package must declare {declaration}"
        );
    }

    let manifest =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-project-manifest/Cargo.toml"))
            .expect("project manifest crate manifest must be readable");
    assert!(
        manifest.contains("yss-computation-settings = { path = \"../yss-computation-settings\" }"),
        "project manifest must consume the canonical computation-settings contract"
    );
    for forbidden in ["chrono", "tauri", "sqlx"] {
        assert!(
            !manifest.contains(forbidden),
            "project manifest must not absorb runtime dependency '{forbidden}'"
        );
    }

    let owner =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-project-manifest/src/lib.rs"))
            .expect("project manifest owner must be readable");
    for contract in [
        "pub const CURRENT_PROJECT_SCHEMA_VERSION",
        "pub struct ProjectManifest",
        "pub fn deserialize_current_project_schema_version",
        "pub fn try_new",
        "pub fn into_parts",
        "computation_settings.validate()?",
        "settings.validate().map_err",
    ] {
        assert!(
            owner.contains(contract),
            "project manifest crate must own strict contract or invariant '{contract}'"
        );
    }
    for mutation_seam in [
        "pub schema_version:",
        "pub project_name:",
        "pub export_time:",
        "pub computation_settings:",
    ] {
        assert!(
            !owner.contains(mutation_seam),
            "validated project manifest state must not expose mutation seam '{mutation_seam}'"
        );
    }
    for misplaced_owner in ["std::fs", "ProjectData", "chrono::", "tauri::", "sqlx::"] {
        assert!(
            !owner.contains(misplaced_owner),
            "project manifest must not absorb I/O/runtime concern '{misplaced_owner}'"
        );
    }

    let project_io = std::fs::read_to_string(root.join("src-tauri/src/project/project_io.rs"))
        .expect("project IO must be readable");
    for removed_root_owner in [
        "pub const SCHEMA_VERSION",
        "pub struct ProjectManifest",
        "deserialize_valid_computation_settings",
        "deserialize_current_schema_version",
    ] {
        assert!(
            !project_io.contains(removed_root_owner),
            "project IO must not retain manifest owner '{removed_root_owner}'"
        );
    }
    assert_eq!(
        project_io.matches("ProjectManifest::try_new").count(),
        1,
        "project IO must use one canonical validated manifest construction seam"
    );

    for relative in [
        "src-tauri/src/project/project_io.rs",
        "src-tauri/src/project/project_lifecycle.rs",
        "src-tauri/src/project/project_state.rs",
        "src-tauri/src/project/project_writers.rs",
        "src-tauri/src/project/project_state/variable_effects.rs",
    ] {
        let consumer = std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("{relative} must be readable: {error}"));
        assert!(
            consumer.contains("yss_project_manifest"),
            "{relative} must consume the canonical project manifest owner directly"
        );
        assert!(
            !consumer.contains("crate::project::ProjectManifest")
                && !consumer.contains("crate::project::project_io::ProjectManifest"),
            "{relative} must not restore a root project-manifest facade"
        );
    }

    let project_module = std::fs::read_to_string(root.join("src-tauri/src/project/mod.rs"))
        .expect("the root project module must be readable");
    assert!(
        !project_module.contains("pub use yss_project_manifest"),
        "the root project module must not restore a project-manifest compatibility facade"
    );

    let policy = std::fs::read_to_string(root.join("src-tauri/src/architecture_tests/policy.rs"))
        .expect("Rust architecture policy must be readable");
    assert!(
        policy.contains("| \"yss-project-manifest\"")
            && policy.contains("layers.insert(RustLayer::PureLeaf)"),
        "project manifest must remain a Pure Leaf contract"
    );
}

#[test]
fn worksheet_document_has_one_strict_pure_crate_owner_without_project_facade() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-worksheet-document/Cargo.toml",
        "src-tauri/crates/yss-worksheet-document/src/lib.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "worksheet document owner must exist at {relative}"
        );
    }
    assert!(
        !root
            .join("src-tauri/src/project/worksheet_resource_path.rs")
            .exists(),
        "Project must not retain the old worksheet resource-path owner"
    );

    let workspace_manifest = std::fs::read_to_string(root.join("src-tauri/Cargo.toml"))
        .expect("the Rust workspace manifest must be readable");
    for declaration in [
        "\"crates/yss-worksheet-document\"",
        "yss-worksheet-document = { path = \"./crates/yss-worksheet-document\" }",
    ] {
        assert!(
            workspace_manifest.contains(declaration),
            "the workspace and root package must declare {declaration}"
        );
    }

    let project_module = std::fs::read_to_string(root.join("src-tauri/src/project/mod.rs"))
        .expect("the root project module must be readable");
    for facade in [
        "mod worksheet_resource_path",
        "pub use worksheet_resource_path",
        "pub use yss_worksheet_document",
    ] {
        assert!(
            !project_module.contains(facade),
            "Project must not restore worksheet compatibility facade '{facade}'"
        );
    }

    let owner =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-worksheet-document/src/lib.rs"))
            .expect("worksheet document owner must be readable");
    for contract in [
        "pub const CURRENT_WORKSHEET_SCHEMA_VERSION",
        "pub struct WorksheetDocument",
        "pub struct WorksheetEncodings",
        "pub struct WorksheetResourcePath",
        "pub enum WorksheetResourcePathError",
        "deserialize_current_schema_version",
    ] {
        assert!(
            owner.contains(contract),
            "worksheet document crate must own canonical contract '{contract}'"
        );
    }
    for layout_mirror in ["pub const WORKSHEETS_DIR", "pub const WORKSHEET_EXTENSION"] {
        assert!(
            !owner.contains(layout_mirror),
            "worksheet document must consume project layout without restoring '{layout_mirror}'"
        );
    }
    assert!(
        owner.contains("yss_project_layout"),
        "worksheet resource paths must consume the canonical project layout"
    );
    assert!(
        owner.matches("deny_unknown_fields").count() >= 2,
        "worksheet document and nested encodings must both reject unknown fields"
    );
    assert!(
        !owner.contains("pub schema_version: u32"),
        "callers must not construct an unsupported worksheet schema version"
    );
    for forbidden in ["std::fs", "tauri"] {
        assert!(
            !owner.contains(forbidden),
            "worksheet document contract must remain platform-neutral without '{forbidden}'"
        );
    }

    let worksheet_io = std::fs::read_to_string(root.join("src-tauri/src/project/worksheet_io.rs"))
        .expect("worksheet IO adapter must be readable");
    for removed_owner in [
        "pub struct WorksheetDocument {",
        "pub struct WorksheetEncodings {",
        "pub struct WorksheetResourcePath {",
        "pub const WORKSHEETS_DIR",
        "pub const WORKSHEET_EXTENSION",
    ] {
        assert!(
            !worksheet_io.contains(removed_owner),
            "Project IO must not restore worksheet contract '{removed_owner}'"
        );
    }

    for relative in [
        "src-tauri/src/application/worksheet.rs",
        "src-tauri/src/commands/command_worksheet.rs",
        "src-tauri/crates/yss-project-history/src/lib.rs",
        "src-tauri/src/project/history_hydration.rs",
        "src-tauri/src/project/project_activation.rs",
        "src-tauri/crates/yss-project-model/src/lib.rs",
        "src-tauri/crates/yss-project-filesystem/src/error.rs",
        "src-tauri/src/project/project_lifecycle.rs",
        "src-tauri/src/project/project_reads.rs",
        "src-tauri/src/project/project_state.rs",
        "src-tauri/src/project/project_writers.rs",
        "src-tauri/crates/yss-resource-lifecycle/src/lib.rs",
        "src-tauri/crates/yss-project-model/src/patch.rs",
        "src-tauri/src/project/resource_reveal.rs",
        "src-tauri/src/project/worksheet_io.rs",
    ] {
        let consumer = std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("{relative} must be readable: {error}"));
        assert!(
            consumer.contains("yss_worksheet_document"),
            "{relative} must consume the worksheet document owner directly"
        );
        for removed_path in [
            "crate::project::WorksheetEncodings",
            "crate::project::WorksheetResourcePath",
            "crate::project::WORKSHEET_",
        ] {
            assert!(
                !consumer.contains(removed_path),
                "{relative} must not restore removed worksheet path '{removed_path}'"
            );
        }
    }

    let policy = std::fs::read_to_string(root.join("src-tauri/src/architecture_tests/policy.rs"))
        .expect("Rust architecture policy must be readable");
    assert!(
        policy.contains("| \"yss-worksheet-document\""),
        "worksheet document must be classified as a Pure Leaf"
    );
    for removed_capability in [
        "yssbi_lib::project::worksheet_io::WorksheetDocument",
        "yssbi_lib::project::worksheet_resource_path::WorksheetResourcePath",
    ] {
        assert!(
            !policy.contains(removed_capability),
            "Commands must not retain Project capability '{removed_capability}'"
        );
    }
}

#[test]
fn project_progress_has_one_pure_crate_owner_without_root_or_stale_event_facades() {
    let root = repository_root();
    for relative in [
        "src-tauri/crates/yss-project-progress/Cargo.toml",
        "src-tauri/crates/yss-project-progress/src/lib.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "project progress owner must exist at {relative}"
        );
    }
    assert!(
        !root
            .join("src-tauri/src/project/project_progress.rs")
            .exists(),
        "the root crate must not retain a project progress owner"
    );
    assert!(
        !root
            .join("src-tauri/src/project/project_picker_task.rs")
            .exists(),
        "the root crate must not retain a project task cancellation owner"
    );

    let workspace_manifest = std::fs::read_to_string(root.join("src-tauri/Cargo.toml"))
        .expect("the Rust workspace manifest must be readable");
    for declaration in [
        "\"crates/yss-project-progress\"",
        "yss-project-progress = { path = \"./crates/yss-project-progress\" }",
    ] {
        assert!(
            workspace_manifest.contains(declaration),
            "the workspace and root package must declare {declaration}"
        );
    }

    let project_module = std::fs::read_to_string(root.join("src-tauri/src/project/mod.rs"))
        .expect("the root project module must be readable");
    for facade in [
        "mod project_progress",
        "pub use project_progress",
        "mod project_picker_task",
        "pub use project_picker_task",
        "pub use yss_project_progress",
    ] {
        assert!(
            !project_module.contains(facade),
            "the root project module must not restore progress facade '{facade}'"
        );
    }

    let progress_owner =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-project-progress/src/lib.rs"))
            .expect("project progress owner must be readable");
    for contract in [
        "pub enum ProjectProgress",
        "pub enum ProjectScanProgress",
        "pub enum ProjectCleanupProgress",
        "pub trait ProjectProgressSink",
        "pub struct ProjectTaskCancellation",
        "pub struct ProjectTaskCancellationRegistry",
    ] {
        assert!(
            progress_owner.contains(contract),
            "project progress crate must own {contract}"
        );
    }
    for platform_dependency in ["serde", "tauri"] {
        assert!(
            !progress_owner.contains(platform_dependency),
            "project progress contract must remain platform-neutral and not depend on {platform_dependency}"
        );
    }

    for relative in [
        "src-tauri/src/lib.rs",
        "src-tauri/crates/yss-project-registry/src/lib.rs",
        "src-tauri/crates/yss-project-discovery/src/lib.rs",
        "src-tauri/src/commands/command_project/registry.rs",
        "src-tauri/src/commands/command_project/progress.rs",
    ] {
        let consumer = std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("{relative} must be readable: {error}"));
        assert!(
            consumer.contains("yss_project_progress::"),
            "{relative} must consume the project progress owner directly"
        );
        assert!(
            !consumer.contains("project::project_progress"),
            "{relative} must not restore the removed root progress path"
        );
    }

    let scan =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-project-discovery/src/lib.rs"))
            .expect("project discovery owner must be readable");
    assert!(
        !scan.contains("ProjectScanProgressEvent"),
        "project scan must not restore the zero-caller duplicate progress DTO"
    );
    for removed_cancellation_implementation in [
        "AtomicBool",
        "PICKER_TASK_CANCELLED",
        "picker_task_cancelled_error",
    ] {
        assert!(
            !scan.contains(removed_cancellation_implementation),
            "Project scan must consume typed cancellation instead of '{removed_cancellation_implementation}'"
        );
    }

    let registry =
        std::fs::read_to_string(root.join("src-tauri/crates/yss-project-registry/src/lib.rs"))
            .expect("project registry source must be readable");
    assert!(
        registry.contains("ProjectDiscoveryError::Cancelled => ProjectRegistryError::Cancelled"),
        "scan cancellation must remain cancellation instead of drifting into ScanFailed"
    );

    let policy = std::fs::read_to_string(root.join("src-tauri/src/architecture_tests/policy.rs"))
        .expect("Rust architecture policy must be readable");
    assert!(
        !policy.contains("yssbi_lib::project::project_picker_task"),
        "CompositionRoot and Commands must not retain a Project task-control capability"
    );

    let command_adapter =
        std::fs::read_to_string(root.join("src-tauri/src/commands/command_project/progress.rs"))
            .expect("project progress command adapter must be readable");
    assert!(
        command_adapter.contains("pub enum ProjectProgressDto"),
        "the Tauri wire projection must remain command-owned"
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
