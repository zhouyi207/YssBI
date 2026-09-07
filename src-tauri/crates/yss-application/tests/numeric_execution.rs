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
