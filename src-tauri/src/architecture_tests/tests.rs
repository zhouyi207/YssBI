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
        "src-tauri/src/project/project_data.rs",
        "src-tauri/src/project/project_io.rs",
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

    let project_io = std::fs::read_to_string(root.join("src-tauri/src/project/project_io.rs"))
        .expect("project IO must be readable");
    for validation_boundary in [
        "deserialize_valid_computation_settings",
        "settings.validate().map_err",
    ] {
        assert!(
            project_io.contains(validation_boundary),
            "project manifest reads must enforce '{validation_boundary}'"
        );
    }
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
        "pub const WORKSHEETS_DIR",
        "pub const WORKSHEET_EXTENSION",
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
        "src-tauri/src/project/history.rs",
        "src-tauri/src/project/history_hydration.rs",
        "src-tauri/src/project/project_activation.rs",
        "src-tauri/src/project/project_data.rs",
        "src-tauri/src/project/project_error.rs",
        "src-tauri/src/project/project_lifecycle.rs",
        "src-tauri/src/project/project_reads.rs",
        "src-tauri/src/project/project_state.rs",
        "src-tauri/src/project/project_writers.rs",
        "src-tauri/src/project/resource_lifecycle.rs",
        "src-tauri/src/project/resource_patch.rs",
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
        "src-tauri/src/project/project_registry.rs",
        "src-tauri/src/project/project_scan.rs",
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

    let scan = std::fs::read_to_string(root.join("src-tauri/src/project/project_scan.rs"))
        .expect("project scan source must be readable");
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

    let registry = std::fs::read_to_string(root.join("src-tauri/src/project/project_registry.rs"))
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
