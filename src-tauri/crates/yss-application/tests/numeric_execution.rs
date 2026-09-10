use std::collections::BTreeMap;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};
use yss_application::graph_contracts::execution_package_from_graph;
use yss_execution::error::RunFailureCode;
use yss_execution::identity::{ExecutionSessionId, RuntimeGeneration};
use yss_execution::plan::{
    PlanCompilationBasis, PlanExecutionDemand, PlanProjectSessionId, PlanRegistryFingerprint,
};
use yss_execution::resource_preparation::{ResourceProviderFactory, RunResourceBindings};
use yss_execution::result::StoredResult;
use yss_execution::state::{ExecutePreparedError, ExecutionRuntimeState, RunExecutionControl};
use yss_execution::value::RuntimeValue;
use yss_graph_analysis_contract::CompileId;
use yss_graph_compiler::{GraphCompilationInput, compile};
use yss_graph_document::{
    ConnectionId, DocumentConnection, DocumentNode, GraphDocument, GraphResourcePath, NodeId,
    NodePosition, ParameterValues, PortAddress,
};
use yss_graph_resource_contract::{ResourceCatalogFingerprint, ResourceCatalogSnapshot};

fn execute(
    document: &GraphDocument,
    output_node_type: &str,
) -> Result<RuntimeValue, ExecutePreparedError> {
    let builtin = yss_graph_catalog::build_builtin_node_system().unwrap();
    let resources = ResourceCatalogSnapshot::new(
        BTreeMap::new(),
        BTreeMap::new(),
        ResourceCatalogFingerprint::from_bytes([0; 32]),
    );
    let semantics =
        yss_graph_analysis::resolve_graph_semantics(document, &builtin.registry, &resources);
    let ready = semantics.ready().unwrap_or_else(|| {
        panic!(
            "the saved graph must pass semantic validation: {:?}",
            semantics.diagnostics()
        )
    });
    let graph = GraphResourcePath::new("events/New Event.yssbi-event").unwrap();
    let package = compile(GraphCompilationInput::new(ready, graph, CompileId::new(1))).unwrap();
    let session = PlanProjectSessionId::from_existing("diagnostic-session".into());
    let basis = PlanCompilationBasis::new(
        session.clone(),
        PlanRegistryFingerprint::from_bytes([0; 32]),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    let package = execution_package_from_graph(package, basis).unwrap();
    let requested_output = package
        .plan()
        .operations()
        .iter()
        .find(|operation| operation.node_type().as_str() == output_node_type)
        .unwrap()
        .outputs()[0]
        .output()
        .clone();
    let state = ExecutionRuntimeState::new(
        ExecutionSessionId::new(uuid::Uuid::new_v4()),
        RuntimeGeneration::INITIAL,
    );
    let plan = state
        .prepare_compiled_package(package, RuntimeGeneration::INITIAL)
        .unwrap();
    let run = state.execute_prepared_handoff(
        &plan,
        RunResourceBindings::new(session, [], []),
        &ResourceProviderFactory::new("diagnostic-session".into()),
        &RunExecutionControl::with_cancellation(
            Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(10),
        ),
        &PlanExecutionDemand::Default,
        |_| {},
    )?;
    let result = run
        .handoff()
        .results()
        .iter()
        .find(|result| result.output() == &requested_output)
        .unwrap();
    let StoredResult::Runtime(value) = result.value().value() else {
        panic!("the requested output must produce a runtime value");
    };
    Ok(value.clone())
}

fn division_graph(denominator: i64) -> (GraphDocument, NodeId) {
    let mut document = GraphDocument::default();
    let left = NodeId::new();
    let right = NodeId::new();
    let divide = NodeId::new();
    for (id, kind) in [
        (left, "yssbi.constant.get"),
        (right, "yssbi.constant.get"),
        (divide, "yssbi.numeric.divide"),
    ] {
        document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: kind.parse().unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
    }
    set_constant(
        &mut document,
        left,
        yss_data_contract::DataType::Float64,
        yss_data_contract::DataValue::Float64(0.0),
    );
    set_constant(
        &mut document,
        right,
        yss_data_contract::DataType::Int64,
        yss_data_contract::DataValue::Int64(denominator),
    );
    for (source, input) in [(left, "left"), (right, "right")] {
        let id = ConnectionId::new();
        document.connections.insert(
            id,
            DocumentConnection {
                id,
                output: PortAddress::declared(source, "value".parse().unwrap()),
                input: PortAddress::declared(divide, input.parse().unwrap()),
                order: None,
            },
        );
    }
    (document, divide)
}

#[test]
fn graph_constants_retain_numeric_types_through_compilation_and_execution() {
    let (document, _) = division_graph(2);
    assert_eq!(
        execute(&document, "yssbi.numeric.divide").unwrap(),
        RuntimeValue::Decimal(0.0)
    );
}

#[test]
fn zero_divisor_reports_the_reason_and_divide_node() {
    let (document, divide) = division_graph(0);
    let failure = execute(&document, "yssbi.numeric.divide")
        .unwrap_err()
        .failure();
    assert_eq!(failure.code, RunFailureCode::DivisionByZero);
    assert_eq!(
        failure.source.unwrap().node().unwrap().as_str(),
        divide.to_string()
    );
}

#[test]
fn dataframe_constants_resolve_column_types_and_execute_without_project_resources() {
    let mut document = GraphDocument::default();
    let source = NodeId::new();
    document.nodes.insert(
        source,
        DocumentNode {
            id: source,
            node_type: "yssbi.constant.get".parse().unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        },
    );
    set_constant(
        &mut document,
        source,
        yss_data_contract::DataType::DataFrame,
        yss_data_contract::DataValue::DataFrame(
            r#"{"amount":[1,2,3],"label":["a","b","c"]}"#.into(),
        ),
    );
    yss_graph_document::normalize_constant_value(document.constants.values_mut().next().unwrap())
        .unwrap();
    let builtin = yss_graph_catalog::build_builtin_node_system().unwrap();
    let resources = ResourceCatalogSnapshot::new(
        BTreeMap::new(),
        BTreeMap::new(),
        ResourceCatalogFingerprint::from_bytes([0; 32]),
    );
    let semantics =
        yss_graph_analysis::resolve_graph_semantics(&document, &builtin.registry, &resources);
    let schema = semantics.nodes()[0].ports[0].schema_state.exact().unwrap();
    assert_eq!(
        schema
            .fields
            .iter()
            .map(|field| (field.name.0.as_ref(), field.scalar_type))
            .collect::<Vec<_>>(),
        [
            ("amount", yss_graph_protocol::RelationalScalarType::Int64),
            ("label", yss_graph_protocol::RelationalScalarType::String),
        ]
    );
    assert_eq!(
        execute(&document, "yssbi.constant.get").unwrap(),
        RuntimeValue::Record(BTreeMap::from([
            (
                "amount".into(),
                RuntimeValue::List(
                    [
                        RuntimeValue::Unsigned(1),
                        RuntimeValue::Unsigned(2),
                        RuntimeValue::Unsigned(3)
                    ]
                    .into()
                )
            ),
            (
                "label".into(),
                RuntimeValue::List(
                    ["a", "b", "c"]
                        .map(|value| RuntimeValue::String(value.into()))
                        .into()
                )
            ),
        ]))
    );
}

fn set_constant(
    document: &mut GraphDocument,
    node: NodeId,
    data_type: yss_data_contract::DataType,
    data_value: yss_data_contract::DataValue,
) {
    let id = yss_graph_document::ConstantId::from_uuid(node.as_uuid());
    document.constants.insert(
        id,
        yss_graph_document::GraphConstant {
            id,
            name: id.to_string(),
            data_type,
            data_value,
            tabular: None,
            description: String::new(),
            tags: vec![],
        },
    );
    let node = document.nodes.get_mut(&node).unwrap();
    node.node_type = "yssbi.constant.get".parse().unwrap();
    node.parameters = ParameterValues::from([(
        "constant".parse().unwrap(),
        serde_json::json!(id.to_string()),
    )]);
}

fn relational_document(resource: &str) -> (GraphDocument, [NodeId; 6]) {
    use yss_graph_document::{DynamicPortBinding, OrderKey, PortInstanceId};
    let mut document = GraphDocument::default();
    let [source, project, filter, response, predictor, fit] =
        std::array::from_fn(|_| NodeId::new());
    for (id, kind, parameters) in [
        (
            source,
            "yssbi.dataframe.source.get",
            vec![("dataframe", serde_json::json!(resource))],
        ),
        (
            project,
            "yssbi.dataframe.project",
            vec![("columns", serde_json::json!(["x", "y"]))],
        ),
        (
            filter,
            "yssbi.dataframe.filter.rows",
            vec![(
                "predicate",
                serde_json::json!({"column":"x","operator":"greaterThan","value":{"type":"integer","value":"2"}}),
            )],
        ),
        (
            response,
            "yssbi.dataframe.series.select",
            vec![("column", serde_json::json!("y"))],
        ),
        (
            predictor,
            "yssbi.dataframe.series.select",
            vec![("column", serde_json::json!("x"))],
        ),
        (fit, "yssbi.statistics.ols.fit", vec![]),
    ] {
        document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: kind.parse().unwrap(),
                position: NodePosition { x: 0., y: 0. },
                parameters: parameters
                    .into_iter()
                    .map(|(key, value)| (key.parse().unwrap(), value))
                    .collect(),
                user_label: None,
            },
        );
    }
    let predictor_port = PortAddress::instance(
        fit,
        "predictors".parse().unwrap(),
        PortInstanceId::from_bytes([7; 16]),
    );
    document.port_bindings.insert(
        predictor_port.clone(),
        DynamicPortBinding::UserCreated {
            order: OrderKey::new("0"),
        },
    );
    let address = |node, key: &str| PortAddress::declared(node, key.parse().unwrap());
    for (output, input) in [
        (address(source, "dataframe"), address(project, "source")),
        (address(project, "result"), address(filter, "source")),
        (address(filter, "result"), address(response, "dataframe")),
        (address(filter, "result"), address(predictor, "dataframe")),
        (address(response, "series"), address(fit, "response")),
        (address(predictor, "series"), predictor_port),
    ] {
        let id = ConnectionId::new();
        document.connections.insert(
            id,
            DocumentConnection {
                id,
                output,
                input,
                order: None,
            },
        );
    }
    (
        document,
        [source, project, filter, response, predictor, fit],
    )
}

#[test]
fn relational_graph_compiles_without_scanning_and_feeds_existing_ols_and_results() {
    use arrow::array::{Float64Array, Int64Array, StringArray};
    use arrow::datatypes::{DataType, Field, Schema};
    use arrow::record_batch::RecordBatch;
    use yss_database_contract::DatabaseId;
    use yss_execution::plan::{
        PlanResourceId, PlanResourceObservedState, PlanResourceRequirement, PlanResourceVersion,
        ResourceAccess, ResourceKind,
    };
    use yss_execution::resource_preparation::RunResourceBinding;
    use yss_graph_resource_contract::{ColumnSchema, DataSchema, GraphResourceId};
    use yss_relational_contract::RelationBinding;

    let (document, _) = relational_document("data");
    let resources = ResourceCatalogSnapshot::new(
        BTreeMap::new(),
        BTreeMap::from([(
            GraphResourceId::new("data"),
            DataSchema {
                columns: ["x", "y", "unused"]
                    .map(|name| ColumnSchema {
                        name: name.into(),
                        data_type: yss_data_contract::DataType::Float64,
                    })
                    .to_vec(),
            },
        )]),
        ResourceCatalogFingerprint::from_bytes([0; 32]),
    );
    let builtins = yss_graph_catalog::build_builtin_node_system().unwrap();
    let semantics =
        yss_graph_analysis::resolve_graph_semantics(&document, &builtins.registry, &resources);
    let ready = semantics
        .ready()
        .unwrap_or_else(|| panic!("{:?}", semantics.diagnostics()));
    let package = compile(GraphCompilationInput::new(
        ready,
        GraphResourcePath::new("events/relational.yssbi-event").unwrap(),
        CompileId::new(1),
    ))
    .unwrap();
    let resource = PlanResourceId::from_existing("data".into());
    let version = PlanResourceVersion::from_existing("7".into());
    let session = PlanProjectSessionId::from_existing("relational-session".into());
    let basis = PlanCompilationBasis::new(
        session.clone(),
        PlanRegistryFingerprint::from_bytes([0; 32]),
        BTreeMap::from([(resource.clone(), version.clone())]),
        BTreeMap::from([(
            resource.clone(),
            PlanResourceObservedState::Present(version.clone()),
        )]),
    );
    let package = execution_package_from_graph(package, basis).unwrap();
    let fit_outputs = package
        .plan()
        .operations()
        .iter()
        .find(|operation| operation.node_type().as_str() == "yssbi.statistics.ols.fit")
        .unwrap()
        .outputs()
        .to_vec();
    let runtime = ExecutionRuntimeState::from_composition(
        ExecutionSessionId::new(uuid::Uuid::new_v4()),
        RuntimeGeneration::INITIAL,
        Arc::new(yss_sci_runtime::SciRuntimeBackend::new()),
    );
    let plan = runtime
        .prepare_compiled_package(package, RuntimeGeneration::INITIAL)
        .unwrap();

    // The full Graph has compiled before its data file exists.
    let directory = std::env::temp_dir().join(format!("yss-relational-{}", uuid::Uuid::new_v4()));
    let path = directory.join("part.parquet");
    struct DatasetLease(std::path::PathBuf);
    impl Drop for DatasetLease {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let lease = Arc::new(DatasetLease(directory));
    let schema = Arc::new(
        yss_tabular_arrow::with_row_columns(
            Schema::new(vec![
                Field::new("x", DataType::Float64, true),
                Field::new("y", DataType::Float64, true),
                Field::new("unused", DataType::Float64, true),
                Field::new("row_id", DataType::Int64, false),
                Field::new("display_order", DataType::Utf8, false),
            ]),
            "row_id",
            "display_order",
        )
        .unwrap(),
    );
    let engine = yss_datafusion::DataFusionRuntime::new(32 * 1024 * 1024, 2).unwrap();
    let relation = engine
        .parquet_relation(
            RelationBinding {
                project_session: "relational-session".into(),
                dataset: DatabaseId::from_existing("data".into()),
                snapshot: "generation-7".into(),
                revision: 7,
            },
            schema.clone(),
            std::slice::from_ref(&path),
            lease.clone(),
        )
        .unwrap();
    let batches = [[4., 5., 6.], [1., 2., 3.]].map(|x| {
        RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(Float64Array::from(x.to_vec())),
                Arc::new(Float64Array::from(x.map(|x| 1. + 2. * x).to_vec())),
                Arc::new(Float64Array::from(vec![0.; 3])),
                Arc::new(Int64Array::from(x.map(|value| 100 - value as i64).to_vec())),
                Arc::new(StringArray::from_iter_values(
                    x.map(|value| format!("{:020}", value as i64)),
                )),
            ],
        )
        .unwrap()
    });
    yss_tabular_io::write_parquet_batches(&path, schema, batches.into_iter().map(Ok)).unwrap();
    let requirement = PlanResourceRequirement::new(
        resource,
        ResourceKind::DataFrame,
        ResourceAccess::Shared,
        false,
    );
    let executed = runtime
        .execute_prepared_handoff(
            &plan,
            RunResourceBindings::new(
                session,
                [requirement.clone()],
                [RunResourceBinding::new(
                    requirement,
                    version,
                    RuntimeValue::Relation(relation),
                )],
            ),
            &ResourceProviderFactory::new("relational-session".into()),
            &RunExecutionControl::with_cancellation(
                Arc::new(AtomicBool::new(false)),
                Instant::now() + Duration::from_secs(30),
            ),
            &PlanExecutionDemand::Default,
            |_| {},
        )
        .unwrap();
    assert!(runtime.publish_committed_results(executed.handoff()));
    let fitted = runtime.query_pin_result(fit_outputs[1].output()).unwrap();
    let StoredResult::Runtime(RuntimeValue::List(fitted)) = fitted.value().value() else {
        panic!("fitted values");
    };
    for (actual, expected) in fitted.iter().zip([7., 9., 11., 13.]) {
        let RuntimeValue::Decimal(actual) = actual else {
            panic!("numeric fitted values");
        };
        assert!(
            (actual - expected).abs() < 1e-10,
            "fitted output must follow the input row order: {fitted:?}"
        );
    }
    assert_eq!(fitted.len(), 4);
    let residuals = runtime.query_pin_result(fit_outputs[2].output()).unwrap();
    let StoredResult::Runtime(RuntimeValue::List(residuals)) = residuals.value().value() else {
        panic!("residuals");
    };
    assert!(
        residuals
            .iter()
            .all(|value| matches!(value, RuntimeValue::Decimal(value) if value.abs() < 1e-10))
    );
    assert!(executed.handoff().results().iter().any(|result| matches!(
        result.value().value(),
        StoredResult::Runtime(RuntimeValue::Relation(_))
    )));
}

#[test]
fn project_dataset_graph_runs_through_application_authority_and_paged_results() {
    use yss_application::execution::result_query::ResultPinQuery;
    use yss_application::execution::run_graph::{RunGraphRequest, run_graph};
    use yss_application::execution::{
        ApplicationSessionEpoch, ApplicationSessionSlot, ApplicationState,
    };
    use yss_application::graph_compile::{CompileGraphDraftReceipt, compile_graph_draft};
    use yss_database_contract::DatabaseImportSource;
    use yss_project::{GraphResourceFile, ProjectState};
    use yss_project_identity::OperationId;

    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let directory = Directory(std::env::temp_dir().join(format!(
        "yss-application-relational-{}",
        uuid::Uuid::new_v4()
    )));
    std::fs::create_dir(&directory.0).unwrap();
    let root = directory.0.join("project");
    let project = Arc::new(ProjectState::new());
    let created = project
        .create_project_transaction("Relational", &root, OperationId::new())
        .unwrap();
    project
        .activate_project_from_path(&created.metadata_path)
        .unwrap();
    let backend = Arc::new(yss_sci_runtime::SciRuntimeBackend::new());
    let candidate = yss_application::execution::session_factory::build_current_project_candidate(
        ApplicationSessionEpoch::INITIAL,
        project.clone(),
        [],
        backend.clone(),
    )
    .unwrap();
    let app = ApplicationState::from_composition(Arc::new(ApplicationSessionSlot::new()), backend);
    app.install_candidate(candidate).unwrap();
    let instance = app.capture_session().unwrap().project_instance_id().clone();
    let csv = directory.0.join("source.csv");
    std::fs::write(
        &csv,
        "x,y,unused\n1,3,0\n2,5,0\n3,7,0\n4,9,0\n5,11,0\n6,13,0\n",
    )
    .unwrap();
    let dataset = app
        .load_database_for_application(
            instance.clone(),
            OperationId::new(),
            DatabaseImportSource::Csv {
                path: csv.to_string_lossy().into(),
                delimiter: ',',
                has_header: true,
                infer_schema_length: Some(10),
            },
        )
        .unwrap()
        .data;
    let resource = format!("databases/{}", dataset.id);
    let (document, [_, _, filter, _, _, fit]) = relational_document(&resource);
    let path = GraphResourcePath::new("events/relational.yssbi-event").unwrap();
    let file = GraphResourceFile {
        kind: yss_graph_document::GraphResourceKind::Event,
        name: "Relational".into(),
        document: document.clone(),
        function: None,
    };
    std::fs::write(root.join(path.as_str()), serde_json::to_vec(&file).unwrap()).unwrap();
    project.load_graph_document(&instance, &path, 1).unwrap();
    let compilation =
        compile_graph_draft(&app, instance.clone(), path.clone(), document, "en").unwrap();
    let CompileGraphDraftReceipt::Ready { artifact_id, .. } = compilation else {
        panic!("project graph did not compile: {compilation:?}");
    };
    let run_id = run_graph(
        &app,
        RunGraphRequest::new(instance, path.clone(), artifact_id),
    )
    .unwrap();
    let query = |node, key: &str| {
        ResultPinQuery::new(
            path.clone(),
            PortAddress::declared(node, key.parse().unwrap()),
        )
    };
    let result = app.query_pin_result(query(fit, "fitted")).unwrap().unwrap();
    assert_eq!(result.provenance().run_id(), run_id);
    let StoredResult::Runtime(RuntimeValue::List(fitted)) = result.value().value() else {
        panic!("fitted values");
    };
    assert_eq!(fitted.len(), 4);
    for (actual, expected) in fitted.iter().zip([7., 9., 11., 13.]) {
        assert!(matches!(actual, RuntimeValue::Decimal(value) if (value - expected).abs() < 1e-10));
    }
    let result = app
        .query_pin_result(query(fit, "residuals"))
        .unwrap()
        .unwrap();
    let StoredResult::Runtime(RuntimeValue::List(residuals)) = result.value().value() else {
        panic!("residuals");
    };
    assert!(
        residuals
            .iter()
            .all(|value| matches!(value, RuntimeValue::Decimal(value) if value.abs() < 1e-10))
    );
    let relation = app
        .query_pin_result(query(filter, "result"))
        .unwrap()
        .unwrap();
    assert!(matches!(
        relation.value().value(),
        StoredResult::Runtime(RuntimeValue::Relation(_))
    ));
    let page = app
        .query_result_page(relation.provenance().result_id(), 0, 2)
        .unwrap()
        .unwrap();
    assert_eq!(page.values.len(), 2);
    assert!(page.has_more);
    assert_eq!(
        page.columns
            .iter()
            .map(|column| column.name.as_ref())
            .collect::<Vec<_>>(),
        vec!["x", "y"]
    );
}
