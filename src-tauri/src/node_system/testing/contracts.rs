use crate::commands::node_system_execution_dto::{
    EXECUTION_DEMAND_DTO_WIRE_TYPES, ExecutionChannelEventDto, ExecutionDemandDto,
    RUN_EVENT_KIND_DTO_WIRE_TYPES, RunEventDto,
};
use crate::event::{
    Event, EventProject, ProjectionStatusDto, ResourceMoveDto, ResourceMutationResultDto,
};
use crate::node_system::ProjectSessionId;
use crate::node_system::analysis::{EditorGraphProjectionDto, ResourceVersionSet};
use crate::node_system::catalog::{
    CatalogResourceEntry, CatalogResourcePath, NodeCreationDescriptor, ResourceBoundCreateArgsDto,
    build_builtin_node_system,
};
use crate::node_system::compiler::{GraphCompiler, ResourceSnapshot};
use crate::node_system::document::{
    DocumentNode, FunctionDocument, FunctionDocumentPatch, FunctionParameter, FunctionParameterId,
    FunctionResourceKey, FunctionSignature, GraphDeltaEvent, GraphDocument, GraphDocumentPatch,
    GraphResourcePath, GraphRevision, HistoryStatusDto, MutationRequest, NodeId, NodePosition,
    OperationId, PortAddress, PortInstanceId, ResourceDocumentPatch, ResourceKey,
    ResourceLifecycleKind, ResourceLifecyclePatch, ResourceLifecycleState, ResourcePathMovePatch,
    ResourceRevision, WorksheetDocumentPatch, WorksheetDocumentState, WorksheetResourceKey,
};
use crate::node_system::plan::{EXECUTION_DEMAND_VARIANT_COUNT, ExecutionDemand, GraphOutputRef};
use crate::node_system::protocol::{NodeTypeId, ParameterKey, PortKey};
use crate::node_system::registry::{NodeRegistry, canonical_semantic_protocol_snapshot};
use crate::node_system::runtime::{
    GraphRunIdentity, OrdinaryRunErrorCode, PlotKind, RUN_EVENT_KIND_VARIANT_COUNT, ResultId,
    RunErrorOutcome, RunEvent, RunEventKind, RunOutputEvent, RunOutputMessage, RunOutputStatus,
    RunOutputStatusEvent, RunOutputStream, RunPhase,
};
use crate::project::{
    GraphDocumentKind, GraphResourceDocument, ProjectData, ProjectGraphIndexEntry,
    ProjectInstanceId, ProjectState,
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

fn catalog_search_wire_contract(
    registry: &NodeRegistry,
    catalog: &crate::node_system::catalog::BuiltinCatalog,
) -> Value {
    let resource = CatalogResourceEntry {
        name: "Straße_Data Cafe\u{301} 数据".into(),
        node_type_id: NodeTypeId::new("yssbi.project.function.call").unwrap(),
        resource_path: CatalogResourcePath::new("functions/catalog-search-wire"),
        resource_revision: ResourceRevision::new(23),
        create_args: ResourceBoundCreateArgsDto::Function,
        technical_terms: vec!["技术_Term".into(), "Maße_Value\u{301}".into()],
    };
    let mut localized = catalog.localize_with_resources(registry, "en-US", &[resource]);
    localized.items.retain(|item| {
        item.node_type_id.as_ref() == "yssbi.numeric.add.int64"
            || item
                .resource_path
                .as_ref()
                .is_some_and(|path| path.as_str() == "functions/catalog-search-wire")
    });
    let category_ids = localized
        .items
        .iter()
        .map(|item| item.category_id.clone())
        .collect::<BTreeSet<_>>();
    localized
        .categories
        .retain(|category| category_ids.contains(&category.category_id));

    serde_json::to_value(localized.into_dto(
        "00000000-0000-0000-0000-000000000017",
        registry.fingerprint().to_hex(),
        23,
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
            parameters: BTreeMap::from([(ParameterKey::new("value").unwrap(), json!(true))]),
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

fn contract_output(port: PortAddress) -> GraphOutputRef {
    GraphOutputRef {
        graph_path: GraphResourcePath(GRAPH_PATH.into()),
        port,
    }
}

fn execution_wire_contract(_registry: &NodeRegistry) -> Value {
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
    let run_id = crate::node_system::runtime::RunId::new(UNSAFE_ID);
    let run = GraphRunIdentity {
        project_session_id: ProjectSessionId::new("contract-session"),
        graph_path: GraphResourcePath(GRAPH_PATH.into()),
        run_id,
    };
    let kinds = [
        RunEventKind::RunStarted,
        RunEventKind::RunCompleted,
        RunEventKind::RunErrored {
            outcome: RunErrorOutcome::Ordinary {
                code: OrdinaryRunErrorCode::KernelFailed,
            },
        },
        RunEventKind::RunErrored {
            outcome: RunErrorOutcome::DeadlineExceeded {
                phase: RunPhase::QueueWait,
            },
        },
        RunEventKind::RunErrored {
            outcome: RunErrorOutcome::DeadlineExceeded {
                phase: RunPhase::Kernel,
            },
        },
        RunEventKind::RunErrored {
            outcome: RunErrorOutcome::DeadlineExceeded {
                phase: RunPhase::StreamSend,
            },
        },
        RunEventKind::RunErrored {
            outcome: RunErrorOutcome::DeadlineExceeded {
                phase: RunPhase::StreamReceive,
            },
        },
        RunEventKind::RunErrored {
            outcome: RunErrorOutcome::DeadlineExceeded {
                phase: RunPhase::AdapterIo,
            },
        },
        RunEventKind::RunErrored {
            outcome: RunErrorOutcome::DeadlineExceeded {
                phase: RunPhase::ResultPublication,
            },
        },
        RunEventKind::RunErrored {
            outcome: RunErrorOutcome::DeadlineExceeded {
                phase: RunPhase::Cleanup,
            },
        },
        RunEventKind::RunCancelled,
        RunEventKind::PinPreviewResultReady {
            output: declared.clone(),
            generation: 17,
            result_id: ResultId::new(UNSAFE_ID),
        },
        RunEventKind::OpenResultWindow {
            result_id: ResultId::new(UNSAFE_ID),
        },
    ];
    let run_events = kinds
        .into_iter()
        .map(|kind| {
            serde_json::to_value(
                RunEventDto::try_from(RunEvent {
                    run: run.clone(),
                    kind,
                })
                .expect("contract run event must convert"),
            )
            .expect("production RunEvent DTO must serialize")
        })
        .collect::<Vec<_>>();
    let source_graph_path = GraphResourcePath("functions/contract-output.yssbi-function".into());
    let source_port = PortAddress::declared(node_id, PortKey::new("message").unwrap());
    let run_output_events = [
        RunOutputMessage::Output(RunOutputEvent {
            run_id,
            sequence: 1,
            stream: RunOutputStream::Stdout,
            text: "stdout value".into(),
            source_graph_path: source_graph_path.clone(),
            source_node_id: node_id,
            source_port: source_port.clone(),
        }),
        RunOutputMessage::Output(RunOutputEvent {
            run_id,
            sequence: 2,
            stream: RunOutputStream::Stderr,
            text: "stderr value".into(),
            source_graph_path: source_graph_path.clone(),
            source_node_id: node_id,
            source_port: source_port.clone(),
        }),
        RunOutputMessage::Status(RunOutputStatusEvent {
            run_id,
            sequence: 3,
            stream: RunOutputStream::Stdout,
            status: RunOutputStatus::Truncated,
            source_graph_path: source_graph_path.clone(),
            source_node_id: node_id,
            source_port: source_port.clone(),
        }),
        RunOutputMessage::Status(RunOutputStatusEvent {
            run_id,
            sequence: 4,
            stream: RunOutputStream::Stdout,
            status: RunOutputStatus::Dropped,
            source_graph_path,
            source_node_id: node_id,
            source_port,
        }),
    ]
    .into_iter()
    .map(|event| {
        serde_json::to_value(ExecutionChannelEventDto::from(event))
            .expect("production run output DTO must serialize")
    })
    .collect::<Vec<_>>();
    let demands = [
        ExecutionDemand::Default,
        ExecutionDemand::Outputs {
            outputs: vec![declared.clone(), instance].into_boxed_slice(),
            include_default_results: false,
        },
        ExecutionDemand::PinPreview {
            output: declared,
            generation: 17,
        },
    ]
    .into_iter()
    .map(|demand| {
        serde_json::to_value(ExecutionDemandDto::from(demand))
            .expect("production execution demand DTO must serialize")
    })
    .collect::<Vec<_>>();

    json!({
        "format": "yssbi.execution-wire.v1",
        "demands": demands,
        "runEvents": run_events,
        "runOutputEvents": run_output_events,
    })
}

fn worksheet_state(
    database_id: &str,
    chart_type: &str,
    x: &str,
    y: &str,
) -> WorksheetDocumentState {
    WorksheetDocumentState {
        database_id: database_id.into(),
        chart_type: chart_type.into(),
        encodings: crate::project::WorksheetEncodings {
            x: Some(x.into()),
            y: Some(y.into()),
        },
    }
}

fn worksheet_result(
    operation: u128,
    publication_revision: u64,
    moves: Vec<ResourceMoveDto>,
    deltas: Vec<crate::node_system::document::ResourceDeltaEvent>,
    history: HistoryStatusDto,
) -> ResourceMutationResultDto {
    ResourceMutationResultDto {
        operation_id: OperationId::from_uuid(Uuid::from_u128(operation)),
        project_instance_id: "00000000-0000-0000-0000-000000000601".into(),
        publication_revision,
        moves,
        deltas,
        projection_replacements: Vec::new(),
        projection_status: ProjectionStatusDto::Complete {
            expected_graph_paths: Vec::new(),
        },
        history,
    }
}

fn project_events_contract() -> Value {
    let operation_id = OperationId::from_uuid(Uuid::from_u128(4));
    let graph_delta = Event::Project(EventProject::GraphDelta {
        project_instance_id: "00000000-0000-0000-0000-000000000601".into(),
        delta: GraphDeltaEvent {
            graph_path: GraphResourcePath(GRAPH_PATH.into()),
            from_revision: ResourceRevision::new(7),
            to_revision: ResourceRevision::new(7),
            caused_by: Some(operation_id),
            payload: GraphDocumentPatch::new([]),
        },
    });
    let created_path = "worksheets/Sales Overview.yssbi-worksheet";
    let renamed_path = "worksheets/Regional Sales.yssbi-worksheet";
    let initial = worksheet_state("database-sales", "scatter", "region", "revenue");
    let saved = worksheet_state("database-sales", "line", "month", "revenue");
    let create_operation = OperationId::from_uuid(Uuid::from_u128(0x901));
    let save_operation = OperationId::from_uuid(Uuid::from_u128(0x902));
    let rename_operation = OperationId::from_uuid(Uuid::from_u128(0x903));
    let remove_operation = OperationId::from_uuid(Uuid::from_u128(0x904));
    let undo_operation = OperationId::from_uuid(Uuid::from_u128(0x905));
    let redo_operation = OperationId::from_uuid(Uuid::from_u128(0x906));
    let lifecycle_state = |revision, path: &str, name: &str| ResourceLifecycleState {
        revision: ResourceRevision::new(revision),
        path: path.into(),
        kind: ResourceLifecycleKind::Worksheet,
        name: name.into(),
    };
    let delta = |operation_id, key: &str, from_revision, to_revision, payload| {
        crate::node_system::document::ResourceDeltaEvent {
            resource: ResourceKey::Worksheet(WorksheetResourceKey(key.into())),
            from_revision: ResourceRevision::new(from_revision),
            to_revision: ResourceRevision::new(to_revision),
            caused_by: Some(operation_id),
            payload,
        }
    };
    let results = vec![
        (
            "create",
            worksheet_result(
                0x901,
                1,
                Vec::new(),
                vec![delta(
                    create_operation,
                    created_path,
                    0,
                    0,
                    ResourceDocumentPatch::ResourceLifecycle(ResourceLifecyclePatch {
                        before: None,
                        after: Some(lifecycle_state(0, created_path, "Sales Overview")),
                    }),
                )],
                HistoryStatusDto {
                    can_undo: true,
                    can_redo: false,
                },
            ),
        ),
        (
            "save",
            worksheet_result(
                0x902,
                2,
                Vec::new(),
                vec![delta(
                    save_operation,
                    created_path,
                    0,
                    1,
                    ResourceDocumentPatch::Worksheet(WorksheetDocumentPatch {
                        before: initial.clone(),
                        after: saved.clone(),
                    }),
                )],
                HistoryStatusDto {
                    can_undo: true,
                    can_redo: false,
                },
            ),
        ),
        (
            "rename",
            worksheet_result(
                0x903,
                3,
                vec![ResourceMoveDto {
                    from: created_path.into(),
                    to: renamed_path.into(),
                    kind: ResourceLifecycleKind::Worksheet,
                    name: "Regional Sales".into(),
                }],
                vec![delta(
                    rename_operation,
                    renamed_path,
                    1,
                    2,
                    ResourceDocumentPatch::ResourceMove(ResourcePathMovePatch {
                        from: created_path.into(),
                        to: renamed_path.into(),
                    }),
                )],
                HistoryStatusDto {
                    can_undo: true,
                    can_redo: false,
                },
            ),
        ),
        (
            "remove",
            worksheet_result(
                0x904,
                4,
                Vec::new(),
                vec![delta(
                    remove_operation,
                    renamed_path,
                    2,
                    3,
                    ResourceDocumentPatch::ResourceLifecycle(ResourceLifecyclePatch {
                        before: Some(lifecycle_state(2, renamed_path, "Regional Sales")),
                        after: None,
                    }),
                )],
                HistoryStatusDto {
                    can_undo: true,
                    can_redo: false,
                },
            ),
        ),
        (
            "undo",
            worksheet_result(
                0x905,
                5,
                Vec::new(),
                vec![delta(
                    undo_operation,
                    renamed_path,
                    3,
                    4,
                    ResourceDocumentPatch::ResourceLifecycle(ResourceLifecyclePatch {
                        before: None,
                        after: Some(lifecycle_state(4, renamed_path, "Regional Sales")),
                    }),
                )],
                HistoryStatusDto {
                    can_undo: true,
                    can_redo: true,
                },
            ),
        ),
        (
            "redo",
            worksheet_result(
                0x906,
                6,
                Vec::new(),
                vec![delta(
                    redo_operation,
                    renamed_path,
                    4,
                    5,
                    ResourceDocumentPatch::ResourceLifecycle(ResourceLifecyclePatch {
                        before: Some(lifecycle_state(4, renamed_path, "Regional Sales")),
                        after: None,
                    }),
                )],
                HistoryStatusDto {
                    can_undo: true,
                    can_redo: false,
                },
            ),
        ),
    ];
    let direct_results = results
        .iter()
        .map(|(scenario, result)| json!({ "scenario": scenario, "result": result }))
        .collect::<Vec<_>>();
    let mutation_events = results
        .into_iter()
        .map(|(_, result)| {
            serde_json::to_value(Event::Project(EventProject::ResourceMutationCommitted {
                result,
            }))
            .expect("production ResourceMutationCommitted event must serialize")
        })
        .collect::<Vec<_>>();
    let mut events = vec![
        serde_json::to_value(graph_delta).expect("production GraphDelta event must serialize"),
    ];
    events.extend(mutation_events);

    json!({
        "format": "yssbi.project-events.v1",
        "resourceMutationResults": direct_results,
        "events": events,
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

fn fingerprint_wire_from_production_encoders(catalog: &Value, editor: &Value) -> Value {
    json!({
        "format": "yssbi.registry-fingerprint-wire.v1",
        "catalog": required_fingerprint(catalog, "/registryFingerprint", "Catalog"),
        "editorProjection": required_fingerprint(
            editor,
            "/basis/registryFingerprint",
            "editor projection",
        ),
    })
}

fn ols_summary_report_contract() -> Value {
    let metadata = crate::sci::models::regression::StatisticalObservationMetadata {
        original_observation_count: 3,
        used_observation_count: 3,
        dropped_null_count: 0,
        dropped_nan_count: 0,
        missing_value_policy: crate::project::StatisticalMissingValuePolicy::Listwise,
        missing_value_policy_source:
            crate::sci::models::regression::StatisticalSettingSource::Project,
        effective_convergence_tolerance: 1e-12,
        convergence_tolerance_source:
            crate::sci::models::regression::StatisticalSettingSource::Project,
        convergence_tolerance_consumed: false,
    };
    use crate::sci::models::regression::{
        LinearRegressionStatistics, RegressionCoefficientStatistics, RegressionStatistics,
    };

    let fit = crate::sci::api::node_statistics::RegressionFit {
        family: "ols",
        coefficients: vec![0.5, 0.75],
        fitted: vec![1.25, 2.0, 2.75],
        residuals: vec![-0.25, 0.0, 0.25],
        statistics: RegressionStatistics::Linear {
            coefficients: RegressionCoefficientStatistics {
                covariance: vec![vec![0.0625, 0.0], vec![0.0, 0.25]],
                standard_errors: vec![0.25, 0.5],
                statistic_values: vec![2.0, 1.5],
                p_values: vec![0.2, 0.1],
                confidence_interval_lower: vec![0.01, -0.23],
                confidence_interval_upper: vec![0.99, 1.73],
            },
            model: LinearRegressionStatistics {
                r2: 0.875,
                adjusted_r2: 0.75,
                f_statistic: 7.0,
                f_p_value: 0.1,
                df_model: 1,
                df_residual: 1,
                df_total: 2,
                ss_model: 1.125,
                ss_residual: 0.125,
                ss_total: 1.25,
                ms_model: 1.125,
                ms_residual: 0.125,
                ms_total: 0.625,
                covariance_type: "nonrobust".to_owned(),
                condition_number: 2.0,
            },
        },
        metadata,
    };
    crate::sci::api::node_statistics::regression_report(&fit)
}

fn contracts() -> BTreeMap<&'static str, Value> {
    let builtin = build_builtin_node_system().expect("built-in node system must validate");
    let registry = builtin.registry;
    let catalog = builtin.catalog;
    let localized_catalog = localized_catalog_contract(&registry, &catalog);
    let editor_projection = editor_projection_contract(&registry, &catalog);
    let fingerprint_wire =
        fingerprint_wire_from_production_encoders(&localized_catalog, &editor_projection);

    BTreeMap::from([
        (
            "semantic-protocol.json",
            serde_json::from_str(&canonical_semantic_protocol_snapshot(&registry).unwrap())
                .unwrap(),
        ),
        ("i18n-inventory.json", i18n_contract(&registry)),
        ("localized-catalog.json", localized_catalog),
        ("ols-summary-report.json", ols_summary_report_contract()),
        (
            "catalog-search-wire.json",
            catalog_search_wire_contract(&registry, &catalog),
        ),
        ("editor-projection.json", editor_projection),
        (
            "function-editor-projection.json",
            function_editor_projection_contract(),
        ),
        ("fingerprint-wire.json", fingerprint_wire),
        ("project-events.json", project_events_contract()),
        ("execution-wire.json", execution_wire_contract(&registry)),
        (
            "plot-payloads.json",
            json!({ "payloads": PlotKind::payload_contract_records() }),
        ),
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
fn function_editor_projection_rejects_empty_struct_keys() {
    for type_name in ["Struct<>", "Struct<   >"] {
        let input = FunctionDocument::new(FunctionSignature {
            parameters: vec![FunctionParameter {
                id: FunctionParameterId("model".into()),
                name: "Model".into(),
                type_name: type_name.into(),
            }],
            return_type: None,
        });
        let output = FunctionDocument::new(FunctionSignature {
            parameters: Vec::new(),
            return_type: Some(type_name.into()),
        });

        assert!(
            crate::node_system::analysis::build_function_editor_projection(&input).is_err(),
            "function editor input accepted invalid type {type_name:?}"
        );
        assert!(
            crate::node_system::analysis::build_function_editor_projection(&output).is_err(),
            "function editor output accepted invalid type {type_name:?}"
        );
    }
}

fn function_editor_projection_contract() -> Value {
    let function = FunctionDocument {
        revision: ResourceRevision::new(1),
        signature: FunctionSignature {
            parameters: vec![FunctionParameter {
                id: FunctionParameterId("sales".into()),
                name: "Observed sales".into(),
                type_name: "DataSeries<Float64>".into(),
            }],
            return_type: Some("Array<String>".into()),
        },
    };
    let function_editor_projection =
        crate::node_system::analysis::build_function_editor_projection(&function)
            .expect("function fixture types must resolve");
    let index = serde_json::to_value(ProjectGraphIndexEntry {
        path: "functions/forecast.yssbi-function".into(),
        name: "Forecast".into(),
        graph_type: GraphDocumentKind::Function,
        revision: ResourceRevision::new(1),
        function_revision: Some(function.revision),
        function_signature: Some(function.signature.clone()),
        function_editor_projection: Some(function_editor_projection.clone()),
    })
    .expect("project index row must serialize");
    let state = ProjectState::new();
    state.activate_project_fixture(
        "function-editor-projection-contract".into(),
        ProjectData::new(),
    );
    let graph_path =
        crate::project::GraphResourcePath::new("functions/forecast.yssbi-function").unwrap();
    state
        .insert_graph(
            graph_path.clone(),
            GraphResourceDocument::new("Forecast", GraphDocumentKind::Function),
        )
        .unwrap();
    let result = state
        .update_function_signature_observed(
            &ProjectInstanceId::from_existing(state.project_instance_id()),
            &graph_path,
            "en-US",
            MutationRequest {
                resource: ResourceKey::Function(FunctionResourceKey(graph_path.as_str().into())),
                base_revision: ResourceRevision::INITIAL,
                operation_id: OperationId::from_uuid(Uuid::from_u128(701)),
                payload: FunctionDocumentPatch::new(
                    FunctionSignature::default(),
                    function.signature.clone(),
                ),
            },
            |_| {},
        )
        .expect("function mutation must publish");
    let replacement = serde_json::to_value(&result.projection_replacements[0])
        .expect("project-event replacement must serialize");
    let expected = json!({
        "functionRevision": 1,
        "inputs": [{
            "id": "sales",
            "name": "Observed sales",
            "dataType": { "kind": "DataSeries", "inner": { "kind": "Float64" } }
        }],
        "outputs": [{
            "id": "return",
            "name": "Array<String>",
            "dataType": { "kind": "Array", "inner": { "kind": "String" } }
        }]
    });

    assert_eq!(index["functionEditorProjection"], expected);
    assert_eq!(replacement["functionEditorProjection"], expected);
    json!({
        "format": "yssbi.function-editor-projection.v1",
        "indexRow": index,
        "replacement": replacement,
    })
}

#[test]
fn function_editor_projection_wire() {
    let contract = function_editor_projection_contract();
    assert_eq!(
        contract["indexRow"]["functionEditorProjection"],
        contract["replacement"]["functionEditorProjection"]
    );
}

#[test]
fn fingerprint_wire_is_extracted_from_production_dto_encoders() {
    let builtin = build_builtin_node_system().expect("built-in node system must validate");
    let catalog = localized_catalog_contract(&builtin.registry, &builtin.catalog);
    let editor = editor_projection_contract(&builtin.registry, &builtin.catalog);

    let wire = fingerprint_wire_from_production_encoders(&catalog, &editor);

    assert_eq!(wire["format"], "yssbi.registry-fingerprint-wire.v1");
    let values = [&wire["catalog"], &wire["editorProjection"]];
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
    let unique_event_types = event_types.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(
        unique_event_types,
        RUN_EVENT_KIND_DTO_WIRE_TYPES.into_iter().collect(),
    );
    assert_eq!(unique_event_types.len(), RUN_EVENT_KIND_VARIANT_COUNT);
    let demand_types = execution["demands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|demand| demand["type"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(demand_types.len(), EXECUTION_DEMAND_VARIANT_COUNT);
    assert_eq!(demand_types, EXECUTION_DEMAND_DTO_WIRE_TYPES);
    for event in run_events {
        assert_eq!(
            event
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["kind".into(), "run".into()]),
        );
        assert_eq!(
            event["run"]
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "graphPath".into(),
                "projectSessionId".into(),
                "runId".into(),
            ]),
        );
    }
    let run_output_events = execution["runOutputEvents"].as_array().unwrap();
    assert_eq!(run_output_events.len(), 4);
    assert_eq!(
        run_output_events
            .iter()
            .map(|event| event["sequence"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        [1, 2, 3, 4]
    );
    assert!(run_output_events.iter().all(|event| {
        event["runId"] == "9007199254740993"
            && event["sourceGraphPath"] == "functions/contract-output.yssbi-function"
            && event["sourceNodeId"] == "00000000-0000-0000-0000-000000000002"
            && event["sourcePort"]
                == json!({
                    "kind": "declared",
                    "nodeId": "00000000-0000-0000-0000-000000000002",
                    "portKey": "message",
                })
    }));
    assert_eq!(run_output_events[0]["stream"], "stdout");
    assert_eq!(run_output_events[1]["stream"], "stderr");
    assert_eq!(run_output_events[2]["status"], "truncated");
    assert_eq!(run_output_events[3]["status"], "dropped");

    let project_events = project_events_contract();
    let direct_results = project_events["resourceMutationResults"]
        .as_array()
        .unwrap();
    assert_eq!(
        direct_results
            .iter()
            .map(|entry| entry["scenario"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["create", "save", "rename", "remove", "undo", "redo"],
    );
    assert_eq!(
        direct_results
            .iter()
            .map(|entry| entry["result"]["deltas"][0]["payload"]["kind"]
                .as_str()
                .unwrap())
            .collect::<Vec<_>>(),
        [
            "resource_lifecycle",
            "worksheet",
            "resource_move",
            "resource_lifecycle",
            "resource_lifecycle",
            "resource_lifecycle",
        ],
    );
    assert_eq!(direct_results[2]["result"]["moves"][0]["kind"], "worksheet");
    assert_eq!(direct_results[4]["result"]["history"]["canRedo"], true);
    assert_eq!(direct_results[5]["result"]["history"]["canRedo"], false);

    let events = project_events["events"].as_array().unwrap();
    assert_eq!(events.len(), 7);
    assert_eq!(events[0]["payload"]["type"], "GraphDelta");
    for (event, direct) in events[1..].iter().zip(direct_results) {
        assert_eq!(event["payload"]["type"], "ResourceMutationCommitted");
        assert_eq!(event["payload"]["payload"]["result"], direct["result"]);
    }
}

#[test]
fn focused_catalog_search_wire_golden_matches_rust_and_is_catalog_only() {
    let builtin = build_builtin_node_system().expect("built-in node system must validate");
    let focused = catalog_search_wire_contract(&builtin.registry, &builtin.catalog);
    let path = fixture_path("catalog-search-wire.json");
    if std::env::var(UPDATE_ENV).as_deref() == Ok("1") {
        write_fixture(&path, &focused);
    }
    let checked_in = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "missing focused Catalog fixture {}: {error}",
            path.display()
        )
    });
    let checked_in: Value = serde_json::from_str(&checked_in).unwrap_or_else(|error| {
        panic!(
            "invalid focused Catalog fixture {}: {error}",
            path.display()
        )
    });

    assert_eq!(checked_in, focused);
    assert_eq!(
        focused
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "categories".into(),
            "items".into(),
            "locale".into(),
            "projectInstanceId".into(),
            "registryFingerprint".into(),
            "resourcePublicationRevision".into(),
        ])
    );
    for item in focused["items"].as_array().unwrap() {
        let fields = item.as_object().unwrap();
        assert!(fields.contains_key("backendSearchText"));
        assert!(fields.contains_key("resourceNames"));
        assert!(!fields.contains_key("pinyin"));
        assert!(!fields.contains_key("searchText"));
        assert!(fields.contains_key("creation"));
        assert!(fields.contains_key("ports"));
        assert!(fields.contains_key("parameters"));
    }
}

#[test]
fn checked_in_node_system_contracts_match_rust() {
    let update = std::env::var(UPDATE_ENV).as_deref() == Ok("1");
    let contracts = contracts();
    for required in [
        "catalog-search-wire.json",
        "project-events.json",
        "execution-wire.json",
        "plot-payloads.json",
    ] {
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
