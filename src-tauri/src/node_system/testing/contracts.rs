use crate::commands::command_trace::TraceRecordDto;
use crate::commands::node_system_execution_dto::RunEventDto;
use crate::node_system::analysis::{
    CompilationBasis, CompileId, CorrelationContext, EditorGraphProjectionDto, ProjectSessionId,
    ResourceVersionSet, SpanEvent, SpanKind, SpanStatus, TraceRecord,
};
use crate::node_system::catalog::{
    CatalogResourceEntry, CatalogResourcePath, NodeCreationDescriptor, ResourceBoundCreateArgsDto,
    build_builtin_node_system,
};
use crate::node_system::compiler::{GraphCompiler, ResourceSnapshot};
use crate::node_system::document::{
    DocumentNode, GraphDocument, GraphResourcePath, GraphRevision, NodeId, NodePosition,
    ResourceRevision,
};
use crate::node_system::protocol::{NodeTypeId, ParameterKey};
use crate::node_system::registry::{NodeRegistry, canonical_semantic_protocol_snapshot};
use crate::node_system::runtime::{RunEvent, RunEventKind};

use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const FIXTURE_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../src/tests/fixtures/node-system-contracts"
);
const UPDATE_ENV: &str = "YSSBI_UPDATE_NODE_CONTRACT_FIXTURES";
const GRAPH_PATH: &str = "events/contract.yssbi-event";

struct ContractResources;

impl ResourceSnapshot for ContractResources {
    fn versions(&self) -> ResourceVersionSet {
        BTreeMap::from([
            (
                crate::node_system::analysis::ResourceKey::new("databases/contract-database"),
                crate::node_system::analysis::ResourceVersion::new("13"),
            ),
            (
                crate::node_system::analysis::ResourceKey::new("functions/contract-function"),
                crate::node_system::analysis::ResourceVersion::new("11"),
            ),
        ])
    }
}

fn i18n_contract(registry: &NodeRegistry) -> Value {
    let required_keys = registry
        .catalog_manifest()
        .i18n
        .keys
        .iter()
        .map(|key| key.as_str())
        .collect::<Vec<_>>();
    let alias_keys = registry
        .iter()
        .filter_map(|(_, node)| node.protocol().catalog.aliases_key.as_ref())
        .map(|key| key.as_str())
        .collect::<BTreeSet<_>>();
    json!({
        "format": "yssbi.i18n-inventory.v1",
        "defaultLocale": "en-US",
        "requiredKeys": required_keys,
        "aliasKeys": alias_keys,
        "registryFingerprint": registry.fingerprint().to_hex(),
    })
}

fn resource(
    name: &str,
    node_type_id: &str,
    resource_path: &str,
    revision: u64,
    create_args: ResourceBoundCreateArgsDto,
) -> CatalogResourceEntry {
    CatalogResourceEntry {
        name: name.into(),
        node_type_id: NodeTypeId::new(node_type_id).unwrap(),
        resource_path: CatalogResourcePath::new(resource_path),
        resource_revision: ResourceRevision::new(revision),
        create_args,
        technical_terms: vec![name.into()],
        pinyin: None,
    }
}

fn localized_catalog_contract(
    registry: &NodeRegistry,
    catalog: &crate::node_system::catalog::BuiltinCatalog,
) -> Value {
    let resources = [
        resource(
            "Contract Database",
            "yssbi.dataframe.source.get",
            "databases/contract-database",
            13,
            ResourceBoundCreateArgsDto::Database,
        ),
        resource(
            "Contract Function",
            "yssbi.project.function.call",
            "functions/contract-function",
            11,
            ResourceBoundCreateArgsDto::Function,
        ),
        resource(
            "Contract Variable",
            "yssbi.project.variable.get",
            "variables/00000000-0000-0000-0000-000000000004",
            12,
            ResourceBoundCreateArgsDto::Variable,
        ),
    ];
    let mut localized = catalog.localize_with_resources(registry, "en-US", &resources);
    localized.items.retain(|item| match &item.creation {
        NodeCreationDescriptor::Static { node_type_id } => {
            node_type_id.as_str() == "yssbi.constant.bool"
        }
        NodeCreationDescriptor::ParameterizedStatic { node_type_id, .. } => {
            node_type_id.as_str() == "yssbi.dataframe.project"
        }
        NodeCreationDescriptor::ResourceBound { resource_path, .. } => resources
            .iter()
            .any(|resource| resource.resource_path == *resource_path),
    });
    serde_json::to_value(localized.into_dto(
        "00000000-0000-0000-0000-000000000001",
        registry.fingerprint().to_hex(),
        17,
    ))
    .unwrap()
}

fn editor_projection_contract(
    registry: &NodeRegistry,
    catalog: &crate::node_system::catalog::BuiltinCatalog,
) -> Value {
    let node_id = NodeId::from_uuid(Uuid::from_u128(2));
    let mut document = GraphDocument::default();
    document.revision = GraphRevision::new(7);
    document.nodes.insert(
        node_id,
        DocumentNode {
            id: node_id,
            node_type: NodeTypeId::new("yssbi.constant.bool").unwrap(),
            position: NodePosition { x: 120.5, y: -32.0 },
            parameters: BTreeMap::from([(
                ParameterKey::new("value").unwrap(),
                json!({ "Bool": true }),
            )]),
            user_label: Some("Contract Boolean".into()),
        },
    );
    let analysis = GraphCompiler::new(registry, &ContractResources)
        .compile(&document)
        .analysis;
    let projection = EditorGraphProjectionDto::from_sources(
        GRAPH_PATH,
        &analysis,
        &document,
        registry,
        &catalog.localization("en-US"),
    )
    .unwrap();
    serde_json::to_value(projection).unwrap()
}

fn contract_correlation(registry: &NodeRegistry) -> CorrelationContext {
    let resource_versions = ContractResources.versions();
    CorrelationContext {
        project_session_id: ProjectSessionId::new("contract-session"),
        graph_path: GraphResourcePath(GRAPH_PATH.into()),
        graph_revision: GraphRevision::new(7),
        registry_fingerprint: registry.fingerprint().clone(),
        resource_versions,
        compile_id: CompileId::new(9),
        selection_digest: Some("contract-selection".into()),
        run_id: None,
        node_id: None,
        node_type_id: None,
        parent_call: None,
    }
}

fn required_fingerprint(value: &Value, pointer: &str, purpose: &str) -> String {
    let fingerprint = value
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{purpose} production DTO is missing string field {pointer}"));
    assert_eq!(
        fingerprint.len(),
        64,
        "{purpose} production fingerprint is not SHA-256 hex"
    );
    assert!(
        fingerprint
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "{purpose} production fingerprint is not lowercase hexadecimal: {fingerprint}"
    );
    fingerprint.to_owned()
}

fn fingerprint_wire_from_production_encoders(
    registry: &NodeRegistry,
    catalog: &Value,
    editor: &Value,
) -> Value {
    let correlation = contract_correlation(registry);
    let basis = CompilationBasis {
        graph_revision: correlation.graph_revision,
        registry_fingerprint: correlation.registry_fingerprint.clone(),
        resource_versions: correlation.resource_versions.clone(),
    };
    let run_event = serde_json::to_value(RunEventDto::from(RunEvent {
        correlation: correlation.clone(),
        basis,
        kind: RunEventKind::RunStarted,
    }))
    .expect("production RunEvent DTO must serialize");
    let trace = serde_json::to_value(TraceRecordDto::from(TraceRecord {
        sequence: 1,
        event: SpanEvent {
            kind: SpanKind::Run,
            status: SpanStatus::Started,
            correlation,
            fields: BTreeMap::new(),
        },
    }))
    .expect("production trace DTO must serialize");

    let run_correlation = required_fingerprint(
        &run_event,
        "/correlation/registryFingerprint",
        "run event correlation",
    );
    assert_eq!(
        run_correlation,
        required_fingerprint(&run_event, "/basis/registryFingerprint", "run event basis"),
        "run event correlation and basis encoders disagree"
    );
    json!({
        "format": "yssbi.registry-fingerprint-wire.v1",
        "catalog": required_fingerprint(catalog, "/registryFingerprint", "Catalog"),
        "editorProjection": required_fingerprint(
            editor,
            "/basis/registryFingerprint",
            "editor projection",
        ),
        "runEvent": run_correlation,
        "trace": required_fingerprint(
            &trace,
            "/correlation/registryFingerprint",
            "trace",
        ),
    })
}

fn contracts() -> BTreeMap<&'static str, Value> {
    let builtin = build_builtin_node_system().expect("built-in node system must validate");
    let registry = builtin.registry;
    let catalog = builtin.catalog;
    let localized_catalog = localized_catalog_contract(&registry, &catalog);
    let editor_projection = editor_projection_contract(&registry, &catalog);
    let fingerprint_wire = fingerprint_wire_from_production_encoders(
        &registry,
        &localized_catalog,
        &editor_projection,
    );

    BTreeMap::from([
        (
            "semantic-protocol.json",
            serde_json::from_str(&canonical_semantic_protocol_snapshot(&registry).unwrap())
                .unwrap(),
        ),
        ("i18n-inventory.json", i18n_contract(&registry)),
        ("localized-catalog.json", localized_catalog),
        ("editor-projection.json", editor_projection),
        ("fingerprint-wire.json", fingerprint_wire),
    ])
}

fn fixture_path(name: &str) -> PathBuf {
    Path::new(FIXTURE_ROOT).join(name)
}

fn write_fixture(path: &Path, value: &Value) {
    fs::create_dir_all(FIXTURE_ROOT).expect("fixture directory must be creatable");
    let mut encoded = serde_json::to_string_pretty(value).expect("fixture must serialize");
    encoded.push('\n');
    fs::write(path, encoded)
        .unwrap_or_else(|error| panic!("failed to update fixture {}: {error}", path.display()));
}

#[test]
fn fingerprint_wire_is_extracted_from_production_dto_encoders() {
    let builtin = build_builtin_node_system().expect("built-in node system must validate");
    let catalog = localized_catalog_contract(&builtin.registry, &builtin.catalog);
    let editor = editor_projection_contract(&builtin.registry, &builtin.catalog);

    let wire = fingerprint_wire_from_production_encoders(&builtin.registry, &catalog, &editor);

    assert_eq!(wire["format"], "yssbi.registry-fingerprint-wire.v1");
    let values = [
        &wire["catalog"],
        &wire["editorProjection"],
        &wire["runEvent"],
        &wire["trace"],
    ];
    for value in values {
        let value = value
            .as_str()
            .expect("production fingerprint field must be a string");
        assert_eq!(value.len(), 64);
        assert!(
            value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            "production fingerprint must be lowercase hexadecimal: {value}"
        );
    }
}

#[test]
fn checked_in_node_system_contracts_match_rust() {
    let update = std::env::var(UPDATE_ENV).as_deref() == Ok("1");
    for (name, generated) in contracts() {
        let path = fixture_path(name);
        if update {
            write_fixture(&path, &generated);
        }
        let checked_in = fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!(
                "missing node-system contract fixture {} ({error}); set {UPDATE_ENV}=1 to update",
                path.display()
            )
        });
        let checked_in: Value = serde_json::from_str(&checked_in)
            .unwrap_or_else(|error| panic!("invalid JSON fixture {}: {error}", path.display()));
        assert_eq!(
            checked_in,
            generated,
            "node-system contract fixture differs: {} (set {UPDATE_ENV}=1 to update)",
            path.display()
        );
    }
}
