use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use syn::{Item, Type};

use super::cargo_targets::rust_workspace_model_from_metadata;
use super::debt::{
    DebtCountDifference, DebtMismatch, compare_exact_rust_debt, rust_architecture_debt,
};
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
    ArchitectureAuditError, ArchitectureFinding, CanonicalDependency, CanonicalOrigin,
    CargoDependencyAuthority, CargoDependencyDeclaration, CargoDependencyScope, DebtKey,
    ExternalDependencyOrigin, ProductionRoot, ProductionRootKind, RustDebtEntry,
    RustDependencyKind, RustDependencyMode, RustLayer, RustModule,
};
use super::policy::{
    InternalDependencyCapability, classify_rust_sources, rust_dependency_findings,
    rust_dependency_findings_with_capabilities,
};
use super::semantic_guards::{graph_document_json_violations, project_to_graph_production_edges};

fn repository_root() -> PathBuf {
    let manifest_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri must have a repository parent")
        .to_path_buf();
    std::fs::canonicalize(manifest_root).expect("repository root must be canonicalizable")
}

fn source_path(relative: &str) -> String {
    repository_root()
        .join(relative)
        .to_string_lossy()
        .into_owned()
}

fn format_debt_mismatch(mismatch: &DebtMismatch, findings: &[ArchitectureFinding]) -> String {
    let mut lines = Vec::new();
    let mut append = |direction: &str, differences: &[DebtCountDifference]| {
        for difference in differences {
            let layers = findings
                .iter()
                .find(|finding| finding.key == difference.key)
                .map(|finding| (finding.source_layer, finding.target_layer));
            lines.push(format!(
                "{direction}|actual={}|declared={}|source_layer={:?}|target_layer={:?}|rule={}|file={}|owner={}|kind={:?}|target={}|locations={:?}|migration={:?}",
                difference.actual_occurrences,
                difference.declared_occurrences,
                layers.map(|(source, _)| source),
                layers.and_then(|(_, target)| target),
                difference.key.rule_id,
                difference.key.repository_relative_source_file,
                difference.key.fully_qualified_owner,
                difference.key.dependency_kind,
                difference.key.canonical_origin_target,
                difference.actual_locations,
                difference.owning_migration_spec,
            ));
        }
    };
    append("new-or-increased", mismatch.new_or_increased());
    append("stale-or-decreased", mismatch.stale_or_decreased());
    lines.join("\n")
}

fn metadata_fixture_with_all_target_kinds() -> Value {
    let yssbi_id = "path+file:///fixture/src-tauri#yssbi@0.3.0";
    let sci_id = "path+file:///fixture/src-tauri/sci#yss-sci@0.1.0";
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
                    {"kind": ["lib"], "name": "yss_sci", "src_path": source_path("src-tauri/sci/src/lib.rs")},
                    {"kind": ["test"], "name": "ignored_sci_test", "src_path": source_path("src-tauri/sci/src/lib.rs")}
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
        std::fs::canonicalize(repository_root().join("src-tauri/sci/src/lib.rs"))
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
            member_package_id: "path+file:///fixture/src-tauri/sci#yss-sci@0.1.0".to_owned()
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
    let build_root = ProductionRoot {
        package_id: "fixture-package".to_owned(),
        package: "fixture".to_owned(),
        target: "build-script-build".to_owned(),
        kind: ProductionRootKind::BuildScript,
        source_path: PathBuf::from("src-tauri/build.rs"),
    };
    let roots = vec![runtime_root.clone(), build_root.clone()];
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
                "src-tauri/src/node_system/catalog/builtin.rs",
                "fixture_lib::node_system::catalog::builtin",
            ),
            module(
                &runtime_root,
                "src-tauri/src/data_contract/mod.rs",
                "fixture_lib::data_contract",
            ),
            module(
                &runtime_root,
                "src-tauri/src/data_contract/data_type.rs",
                "fixture_lib::data_contract::data_type",
            ),
            module(
                &runtime_root,
                "src-tauri/src/data_contract/data_value.rs",
                "fixture_lib::data_contract::data_value",
            ),
            module(
                &runtime_root,
                "src-tauri/src/graph/value/type_system.rs",
                "fixture_lib::graph::value::type_system",
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
        classified["src-tauri/src/node_system/catalog/builtin.rs"],
        RustLayer::BuiltinComposition
    );
    assert_eq!(
        classified["src-tauri/src/data_contract/mod.rs"],
        RustLayer::PureLeaf
    );
    assert_eq!(
        classified["src-tauri/src/data_contract/data_type.rs"],
        RustLayer::PureLeaf
    );
    assert_eq!(
        classified["src-tauri/src/data_contract/data_value.rs"],
        RustLayer::PureLeaf
    );
    assert_eq!(
        classified["src-tauri/src/graph/value/type_system.rs"],
        RustLayer::Graph
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
        "the real production graph must exercise all fifteen Rust layers"
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
    let [crate_root, contract, symbol] = segments.as_slice() else {
        return None;
    };
    if type_path.path.leading_colon.is_some()
        || crate_root != "crate"
        || contract != "data_contract"
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
fn persisted_data_contract_preserves_wire_and_uses_typed_parse_errors() {
    use crate::data_contract::{
        CategoricalRole, DataSeriesValue, DataType, DataTypeParseError, DataValue, DummyInfo,
        TimeSeriesState,
    };

    let id_only = DataValue::DataSeries(DataSeriesValue::new("series-id"));
    assert_eq!(
        serde_json::to_value(&id_only).expect("id-only data series must serialize"),
        serde_json::json!({"DataSeries": "series-id"})
    );

    let full = DataValue::DataSeries(DataSeriesValue {
        id: "series-id".to_owned(),
        element_type: Some(DataType::String),
        dummy_info: Some(DummyInfo {
            drop_category: Some("baseline".to_owned()),
            role: CategoricalRole::Individual,
        }),
        time_series_state: Some(TimeSeriesState::Aligned),
    });
    let expected = serde_json::json!({
        "DataSeries": {
            "id": "series-id",
            "elementType": {"kind": "String"},
            "dummyInfo": {
                "dropCategory": "baseline",
                "role": "individual"
            },
            "timeSeriesState": "aligned"
        }
    });
    assert_eq!(
        serde_json::to_value(&full).expect("full data series must serialize"),
        expected
    );
    assert_eq!(
        serde_json::from_value::<DataValue>(expected)
            .expect("persisted full data series must deserialize"),
        full
    );

    assert_eq!("".parse::<DataType>(), Err(DataTypeParseError::Empty));
    assert_eq!(
        "Array<Int64".parse::<DataType>(),
        Err(DataTypeParseError::MalformedComposite)
    );
    assert_eq!(
        "Unknown".parse::<DataType>(),
        Err(DataTypeParseError::UnknownKind)
    );
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
        "src-tauri/src/data_contract/mod.rs",
        "src-tauri/src/data_contract/data_type.rs",
        "src-tauri/src/data_contract/data_value.rs",
        "src-tauri/src/graph_document/mod.rs",
        "src-tauri/src/graph_document/identity.rs",
        "src-tauri/src/graph_document/model.rs",
        "src-tauri/src/graph_document/resource_path.rs",
        "src-tauri/src/node_system/protocol/identity.rs",
        "src-tauri/src/node_system/protocol/types.rs",
    ] {
        assert_eq!(classification.get(source), Some(&RustLayer::PureLeaf));
    }
    assert_eq!(
        classification.get("src-tauri/src/graph/value/type_system.rs"),
        Some(&RustLayer::Graph),
        "Graph-owned value behavior must not inherit the Pure Leaf contract classification"
    );
    assert!(
        modules.iter().all(|module| {
            !matches!(
                module.repository_relative_source_file.as_str(),
                "src-tauri/src/graph/value/data_type.rs"
                    | "src-tauri/src/graph/value/data_value.rs"
            )
        }),
        "the old Graph value declarations must not remain production modules"
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
                } if repository_relative_declaration_file.starts_with("src-tauri/src/data_contract/")
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
            required_origin: "src-tauri/src/data_contract/data_type.rs",
            allowed_origins: &["src-tauri/src/data_contract/data_type.rs"],
        },
        CanonicalOwnerExpectation {
            symbol: "DataValue",
            required_origin: "src-tauri/src/data_contract/data_value.rs",
            allowed_origins: &["src-tauri/src/data_contract/data_value.rs"],
        },
        CanonicalOwnerExpectation {
            symbol: "DataSeriesValue",
            required_origin: "src-tauri/src/data_contract/data_value.rs",
            allowed_origins: &["src-tauri/src/data_contract/data_value.rs"],
        },
        CanonicalOwnerExpectation {
            symbol: "CategoricalRole",
            required_origin: "src-tauri/src/data_contract/data_value.rs",
            allowed_origins: &[
                "src-tauri/src/data_contract/data_value.rs",
                "src-tauri/src/sci/api/computation.rs",
            ],
        },
        CanonicalOwnerExpectation {
            symbol: "TimeSeriesState",
            required_origin: "src-tauri/src/data_contract/data_value.rs",
            allowed_origins: &["src-tauri/src/data_contract/data_value.rs"],
        },
        CanonicalOwnerExpectation {
            symbol: "DummyInfo",
            required_origin: "src-tauri/src/data_contract/data_value.rs",
            allowed_origins: &["src-tauri/src/data_contract/data_value.rs"],
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
    assert_eq!(
        graph_document_json_violations(&repository_root()),
        Vec::<String>::new()
    );
}

#[test]
fn rust_project_production_does_not_depend_on_graph_layer() {
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

    assert_eq!(
        project_to_graph_production_edges(&dependencies, &classification),
        Vec::<String>::new()
    );
}

#[test]
fn categorical_role_owner_policy_requires_persisted_owner_and_only_approved_sci_origin() {
    let expectation = CanonicalOwnerExpectation {
        symbol: "CategoricalRole",
        required_origin: "src-tauri/src/data_contract/data_value.rs",
        allowed_origins: &[
            "src-tauri/src/data_contract/data_value.rs",
            "src-tauri/src/sci/api/computation.rs",
        ],
    };

    assert!(canonical_owner_origins_are_valid(
        &expectation,
        &BTreeSet::from([
            "src-tauri/src/data_contract/data_value.rs",
            "src-tauri/src/sci/api/computation.rs",
        ]),
    ));
    assert!(!canonical_owner_origins_are_valid(
        &expectation,
        &BTreeSet::from([
            "src-tauri/src/data_contract/data_value.rs",
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
                "src-tauri/src/data_contract/data_value.rs",
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
        BTreeSet::from(["src-tauri/src/data_contract/data_value.rs"]),
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
                        == "src-tauri/src/data_contract/data_value.rs"
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
pub type PersistedDataType = crate::data_contract::DataType;
pub type PersistedDataValue = crate::data_contract::DataValue;
pub type PersistedDataSeriesValue = crate::data_contract::DataSeriesValue;
pub type PersistedCategoricalRole = crate::data_contract::CategoricalRole;
pub type PersistedTimeSeriesState = crate::data_contract::TimeSeriesState;
pub type PersistedDummyInfo = crate::data_contract::DummyInfo;

#[cfg(test)]
pub type TestOnlyAlias = crate::data_contract::DataType;
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

pub type PersistedCategoricalRole = crate::data_contract::CategoricalRole;
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
            "src-tauri/src/graph/value/aliases.rs|PersistedCategoricalRole|crate::data_contract::CategoricalRole",
            "src-tauri/src/graph/value/aliases.rs|PersistedDataSeriesValue|crate::data_contract::DataSeriesValue",
            "src-tauri/src/graph/value/aliases.rs|PersistedDataType|crate::data_contract::DataType",
            "src-tauri/src/graph/value/aliases.rs|PersistedDataValue|crate::data_contract::DataValue",
            "src-tauri/src/graph/value/aliases.rs|PersistedDummyInfo|crate::data_contract::DummyInfo",
            "src-tauri/src/graph/value/aliases.rs|PersistedTimeSeriesState|crate::data_contract::TimeSeriesState",
            "src-tauri/src/sci/api/computation.rs|PersistedCategoricalRole|crate::data_contract::CategoricalRole",
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
            "src-tauri/sci/src/api/computation.rs".to_owned(),
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
            repository_relative_declaration_file: "src-tauri/sci/src/api/computation.rs".to_owned(),
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
fn rust_exact_debt_detects_both_directions() {
    const MIGRATION_SPEC: &str = "docs/architecture/RUST_BACKEND_ADAPTER_BOUNDARIES.md";
    let key = DebtKey {
        rule_id: "rust.internal.source-layer".to_owned(),
        repository_relative_source_file: "src-tauri/src/project/project_state.rs".to_owned(),
        fully_qualified_owner: "yssbi_lib::project::project_state".to_owned(),
        dependency_kind: RustDependencyKind::Use,
        canonical_origin_target: "yssbi_lib::node_system::runtime::RunState".to_owned(),
    };
    let finding = |line| ArchitectureFinding {
        key: key.clone(),
        source_layer: RustLayer::Project,
        target_layer: Some(RustLayer::Execution),
        line,
        column: 1,
    };
    let actual = vec![finding(7), finding(19)];

    let new_or_increased = compare_exact_rust_debt(
        &actual,
        &[RustDebtEntry {
            key: key.clone(),
            expected_occurrences: 1,
            owning_migration_spec: MIGRATION_SPEC,
        }],
    )
    .expect_err("actual count above the declaration must fail");
    assert_eq!(new_or_increased.new_or_increased()[0].actual_occurrences, 2);
    assert!(format_debt_mismatch(&new_or_increased, &actual).contains(
        "new-or-increased|actual=2|declared=1|source_layer=Some(Project)|target_layer=Some(Execution)"
    ));
    assert_eq!(
        new_or_increased.new_or_increased()[0].declared_occurrences,
        1
    );

    let stale_or_decreased = compare_exact_rust_debt(
        &actual,
        &[RustDebtEntry {
            key: key.clone(),
            expected_occurrences: 3,
            owning_migration_spec: MIGRATION_SPEC,
        }],
    )
    .expect_err("declared count above reality must fail as stale debt");
    assert_eq!(
        stale_or_decreased.stale_or_decreased()[0].actual_occurrences,
        2
    );
    assert_eq!(
        stale_or_decreased.stale_or_decreased()[0].declared_occurrences,
        3
    );

    let mut moved_key = key.clone();
    moved_key.repository_relative_source_file =
        "src-tauri/src/project/moved_project_state.rs".to_owned();
    let moved = compare_exact_rust_debt(
        &actual,
        &[RustDebtEntry {
            key: moved_key,
            expected_occurrences: 2,
            owning_migration_spec: MIGRATION_SPEC,
        }],
    )
    .expect_err("a source move must create one new key and one stale key");
    assert_eq!(moved.new_or_increased().len(), 1);
    assert_eq!(moved.stale_or_decreased().len(), 1);
}

#[test]
fn rust_debt_references_maintained_architecture_documents() {
    let specs = rust_architecture_debt()
        .into_iter()
        .map(|entry| entry.owning_migration_spec)
        .collect::<std::collections::BTreeSet<_>>();

    assert!(
        !specs.is_empty(),
        "the current debt manifest must be explicit"
    );
    for spec in specs {
        assert!(
            spec.starts_with("docs/architecture/"),
            "architecture debt must reference maintained documentation: {spec}"
        );
        assert!(
            repository_root().join(spec).is_file(),
            "architecture debt document does not exist: {spec}"
        );
    }
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

    let declared_debt = rust_architecture_debt();
    if let Err(mismatch) = compare_exact_rust_debt(&findings, &declared_debt) {
        panic!(
            "real Rust architecture debt differs from its literal manifest:\n{}",
            format_debt_mismatch(&mismatch, &findings)
        );
    }
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
