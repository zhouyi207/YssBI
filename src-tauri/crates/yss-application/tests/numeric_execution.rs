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

fn execute(document: &GraphDocument) -> Result<RuntimeValue, ExecutePreparedError> {
    let builtin = yss_graph_catalog::build_builtin_node_system().unwrap();
    let resources = ResourceCatalogSnapshot::new(
        BTreeMap::new(),
        BTreeMap::new(),
        BTreeMap::new(),
        ResourceCatalogFingerprint::from_bytes([0; 32]),
    );
    let semantics =
        yss_graph_analysis::resolve_graph_semantics(document, &builtin.registry, &resources);
    let ready = semantics
        .ready()
        .expect("the saved graph must pass semantic validation");
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
    let divide_output = package
        .plan()
        .operations()
        .iter()
        .find(|operation| operation.node_type().as_str() == "yssbi.numeric.divide")
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
        .find(|result| result.output() == &divide_output)
        .unwrap();
    let StoredResult::Runtime(value) = result.value().value() else {
        panic!("numeric division must produce a runtime scalar");
    };
    Ok(value.clone())
}

fn division_graph(denominator: i64) -> (GraphDocument, NodeId) {
    let mut document = GraphDocument::default();
    let left = NodeId::new();
    let right = NodeId::new();
    let divide = NodeId::new();
    for (id, kind) in [
        (left, "yssbi.constant.float64"),
        (right, "yssbi.constant.int64"),
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
    document
        .nodes
        .get_mut(&right)
        .unwrap()
        .parameters
        .insert("value".parse().unwrap(), serde_json::json!(denominator));
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
fn implicit_float_default_remains_numeric_through_compilation_and_execution() {
    let (document, _) = division_graph(2);
    assert_eq!(execute(&document).unwrap(), RuntimeValue::Decimal(0.0));
}

#[test]
fn zero_divisor_reports_the_reason_and_divide_node() {
    let (document, divide) = division_graph(0);
    let failure = execute(&document).unwrap_err().failure();
    assert_eq!(failure.code, RunFailureCode::DivisionByZero);
    assert_eq!(
        failure.source.unwrap().node().unwrap().as_str(),
        divide.to_string()
    );
}
