use crate::commands::command_node_system::ExecuteGraphResultDto;
use crate::commands::command_trace::TraceRecordDto;
use crate::commands::node_system_execution_dto::{
    EXECUTION_DEMAND_DTO_WIRE_TYPES, ExecutionDemandDto, RUN_EVENT_KIND_DTO_WIRE_TYPES, RunEventDto,
};
use crate::event::{Event, EventProject, ProjectionStatusDto, ResourceMutationResultDto};
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
    DocumentNode, GraphDeltaEvent, GraphDocument, GraphDocumentPatch, GraphResourcePath,
    GraphRevision, HistoryStatusDto, NodeId, NodePosition, OperationId, PortAddress,
    PortInstanceId, ResourceRevision,
};
use crate::node_system::plan::{EXECUTION_DEMAND_VARIANT_COUNT, ExecutionDemand, GraphOutputRef};
use crate::node_system::protocol::{NodeTypeId, ParameterKey, PortKey};
use crate::node_system::registry::{NodeRegistry, canonical_semantic_protocol_snapshot};
use crate::node_system::runtime::{
    RUN_EVENT_KIND_VARIANT_COUNT, ResultSourceId, RunErrorCode, RunEvent, RunEventKind,
};

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

fn contract_output(port: PortAddress) -> GraphOutputRef {
    GraphOutputRef {
        graph_path: GraphResourcePath(GRAPH_PATH.into()),
        port,
    }
}

fn execution_wire_contract(registry: &NodeRegistry) -> Value {
    const UNSAFE_ID: u64 = 9_007_199_254_740_993;
    let node_id = NodeId::from_uuid(Uuid::from_u128(2));
    let declared = contract_output(PortAddress::declared(
        node_id,
        PortKey::new("result").unwrap(),
    ));
    let instance = contract_output(PortAddress::instance(
        node_id,
        PortKey::new("results").unwrap(),
        PortInstanceId::from_uuid(Uuid::from_u128(3)),
    ));
    let mut correlation = contract_correlation(registry);
    correlation.compile_id = CompileId::new(UNSAFE_ID);
    correlation.run_id = Some(crate::node_system::analysis::RunId::new(UNSAFE_ID));
    let basis = CompilationBasis {
        graph_revision: correlation.graph_revision,
        registry_fingerprint: correlation.registry_fingerprint.clone(),
        resource_versions: correlation.resource_versions.clone(),
    };
    let kinds = [
        RunEventKind::RunStarted,
        RunEventKind::RunCompleted,
        RunEventKind::RunErrored {
            code: RunErrorCode::KernelFailed,
        },
        RunEventKind::RunCancelled,
        RunEventKind::OperationStarted {
            operation_index: 3,
            activation_id: UNSAFE_ID,
        },
        RunEventKind::OperationCompleted {
            operation_index: 3,
            activation_id: UNSAFE_ID,
        },
        RunEventKind::OperationErrored {
            operation_index: 3,
            activation_id: UNSAFE_ID,
            code: RunErrorCode::KernelFailed,
        },
        RunEventKind::ValueReady {
            value_index: 4,
            source_id: ResultSourceId::new(UNSAFE_ID),
        },
        RunEventKind::ResultReady {
            name: "contract-result".into(),
            source_id: ResultSourceId::new(UNSAFE_ID),
        },
        RunEventKind::OutputReady {
            output: declared.clone(),
            source_id: ResultSourceId::new(UNSAFE_ID),
        },
    ];
    let run_events = kinds
        .into_iter()
        .map(|kind| {
            serde_json::to_value(RunEventDto::from(RunEvent {
                correlation: correlation.clone(),
                basis: basis.clone(),
                kind,
            }))
            .expect("production RunEvent DTO must serialize")
        })
        .collect::<Vec<_>>();
    let demands = [
        ExecutionDemand::Default,
        ExecutionDemand::Outputs {
            outputs: vec![declared, instance].into_boxed_slice(),
            include_default_results: false,
        },
    ]
    .into_iter()
    .map(|demand| {
        serde_json::to_value(ExecutionDemandDto::from(demand))
            .expect("production execution demand DTO must serialize")
    })
    .collect::<Vec<_>>();
    let execute_graph_result = serde_json::to_value(ExecuteGraphResultDto {
        run_id: UNSAFE_ID.to_string(),
    })
    .expect("production execute graph result DTO must serialize");

    json!({
        "format": "yssbi.execution-wire.v1",
        "demands": demands,
        "runEvents": run_events,
        "executeGraphResult": execute_graph_result,
    })
}

fn project_events_contract() -> Value {
    let operation_id = OperationId::from_uuid(Uuid::from_u128(4));
    let graph_delta = Event::Project(EventProject::GraphDelta {
        project_instance_id: "00000000-0000-0000-0000-000000000601".into(),
        delta: GraphDeltaEvent {
            graph_path: GraphResourcePath(GRAPH_PATH.into()),
            from_revision: ResourceRevision::new(7),
            to_revision: ResourceRevision::new(8),
            caused_by: Some(operation_id),
            payload: GraphDocumentPatch::new([]),
        },
    });
    let resource_mutation = Event::Project(EventProject::ResourceMutationCommitted {
        result: ResourceMutationResultDto {
            operation_id,
            project_instance_id: "00000000-0000-0000-0000-000000000601".into(),
            publication_revision: 11,
            moves: Vec::new(),
            deltas: Vec::new(),
            worksheet_deltas: Vec::new(),
            projection_replacements: Vec::new(),
            projection_status: ProjectionStatusDto::Complete {
                expected_graph_paths: Vec::new(),
            },
            history: HistoryStatusDto {
                can_undo: true,
                can_redo: false,
            },
        },
    });

    json!({
        "format": "yssbi.project-events.v1",
        "events": [
            serde_json::to_value(graph_delta).expect("production GraphDelta event must serialize"),
            serde_json::to_value(resource_mutation)
                .expect("production ResourceMutationCommitted event must serialize"),
        ],
    })
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
        ("project-events.json", project_events_contract()),
        ("execution-wire.json", execution_wire_contract(&registry)),
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
fn execution_and_project_event_contract_inventories_are_complete() {
    let builtin = build_builtin_node_system().expect("built-in node system must validate");
    let execution = execution_wire_contract(&builtin.registry);
    let run_events = execution["runEvents"].as_array().unwrap();
    // Source declarations emit these counts; adding a source variant also breaks the
    // exhaustive DTO conversion until both conversion and fixture coverage are updated.
    let event_types = run_events
        .iter()
        .map(|event| event["kind"]["type"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(run_events.len(), RUN_EVENT_KIND_VARIANT_COUNT);
    assert_eq!(event_types, RUN_EVENT_KIND_DTO_WIRE_TYPES);
    let demand_types = execution["demands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|demand| demand["type"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(demand_types.len(), EXECUTION_DEMAND_VARIANT_COUNT);
    assert_eq!(demand_types, EXECUTION_DEMAND_DTO_WIRE_TYPES);
    assert_eq!(execution["executeGraphResult"]["runId"], "9007199254740993");
    for event in run_events {
        assert_eq!(
            event
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["basis".into(), "correlation".into(), "kind".into()]),
        );
    }

    let project_events = project_events_contract();
    let events = project_events["events"].as_array().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["payload"]["type"], "GraphDelta");
    assert_eq!(events[1]["payload"]["type"], "ResourceMutationCommitted");
}

#[test]
fn checked_in_node_system_contracts_match_rust() {
    let update = std::env::var(UPDATE_ENV).as_deref() == Ok("1");
    let contracts = contracts();
    for required in ["project-events.json", "execution-wire.json"] {
        assert!(
            contracts.contains_key(required),
            "missing required node-system contract {required}"
        );
    }
    for (name, generated) in contracts {
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
