use std::collections::BTreeMap;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};
use yss_graph_document::{
    ConnectionId, DocumentConnection, DocumentNode, GraphDocument, GraphResourcePath, NodeId,
    NodePosition, ParameterValues, PortAddress,
};
use yss_graph_execution::error::RunFailureCode;
use yss_graph_execution::identity::{ExecutionSessionId, RuntimeGeneration};
use yss_graph_execution::plan::{
    PlanBasis, PlanExecutionDemand, PlanProjectSessionId, PlanRegistryFingerprint,
};
use yss_graph_execution::resource_preparation::{ResourceProviderFactory, RunResourceBindings};
use yss_graph_execution::state::{
    ExecutePreparedError, ExecutionRuntimeState, RunExecutionControl,
};
use yss_graph_resource_contract::{ResourceCatalogFingerprint, ResourceCatalogSnapshot};
use yss_node_kernel::RuntimeValue;

fn analyze_document(
    document: &GraphDocument,
    graph: &GraphResourcePath,
    resources: &ResourceCatalogSnapshot,
) -> yss_graph_analysis::GraphAnalysis {
    let builtin = yss_node_catalog::build_builtin_node_system().unwrap();
    let graph_runtime = yss_graph_runtime::GraphRuntimeState::from_components(
        yss_graph_runtime::GraphRuntimeEpoch::from_existing(1),
        yss_graph_runtime::GraphRuntimeComponents {
            registry: builtin.registry,
            catalog: builtin.catalog,
        },
    )
    .unwrap();
    graph_runtime.resolve_graph_document(
        graph,
        document,
        &yss_graph_analysis_contract::GraphAnalysisBasis {
            registry_fingerprint: yss_node_registry::RegistryFingerprint::from_bytes(
                graph_runtime.registry_fingerprint(),
            ),
            kernel_fingerprint: yss_node_kernel::KernelRegistry::default()
                .fingerprint()
                .as_bytes(),
            resource_versions: BTreeMap::new(),
            resource_observations: BTreeMap::new(),
        },
        resources,
        &[],
        "en-US",
    )
}

fn execute(
    document: &GraphDocument,
    output_node_type: &str,
) -> Result<RuntimeValue, ExecutePreparedError> {
    let resources = ResourceCatalogSnapshot::new(
        BTreeMap::new(),
        BTreeMap::new(),
        ResourceCatalogFingerprint::from_bytes([0; 32]),
    );
    let graph = GraphResourcePath::new("events/New Event.yssbi-event").unwrap();
    let analysis = analyze_document(document, &graph, &resources);
    let session = PlanProjectSessionId::from_existing("diagnostic-session".into());
    let state = ExecutionRuntimeState::new(
        ExecutionSessionId::new(uuid::Uuid::new_v4()),
        RuntimeGeneration::INITIAL,
        yss_node_kernel::KernelRegistry::default().into(),
    );
    let basis = PlanBasis::new(
        session.clone(),
        PlanRegistryFingerprint::from_bytes([0; 32]),
        state.kernels().fingerprint(),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    let package = state
        .prepare_graph_package(&graph, &analysis, basis)
        .unwrap();
    let requested_output = package
        .plan()
        .operations()
        .iter()
        .find(|operation| operation.node_type().as_str() == output_node_type)
        .unwrap()
        .outputs()[0]
        .output()
        .clone();
    let plan = state
        .prepare_package(package, RuntimeGeneration::INITIAL)
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
        None,
        |_| {},
    )?;
    let result = run
        .handoff()
        .results()
        .iter()
        .find(|result| result.output() == &requested_output)
        .unwrap();
    Ok(result.value().value().clone())
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
        yss_data_contract::ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
        yss_data_contract::DataValue::Float64(0.0),
    );
    set_constant(
        &mut document,
        right,
        yss_data_contract::ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
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
fn graph_constants_retain_numeric_types_through_preparation_and_execution() {
    let (mut document, divide) = division_graph(2);
    assert_eq!(
        execute(&document, "yssbi.numeric.divide").unwrap(),
        RuntimeValue::Decimal(0.0)
    );
    document
        .constants
        .values_mut()
        .find(|constant| {
            matches!(
                constant.data_value,
                yss_data_contract::DataValue::Float64(_)
            )
        })
        .unwrap()
        .data_value = yss_data_contract::DataValue::Float64(8.0);
    let multiply = NodeId::new();
    document.nodes.insert(
        multiply,
        DocumentNode {
            id: multiply,
            node_type: "yssbi.numeric.multiply".parse().unwrap(),
            position: NodePosition { x: 400., y: 0. },
            parameters: ParameterValues::new(),
            user_label: None,
        },
    );
    for input in ["left", "right"] {
        let id = ConnectionId::new();
        document.connections.insert(
            id,
            DocumentConnection {
                id,
                output: PortAddress::declared(divide, "result".parse().unwrap()),
                input: PortAddress::declared(multiply, input.parse().unwrap()),
                order: None,
            },
        );
    }
    assert_eq!(
        execute(&document, "yssbi.numeric.multiply").unwrap(),
        RuntimeValue::Decimal(16.0)
    );

    let (mut wide, operation) = division_graph(1);
    wide.nodes.get_mut(&operation).unwrap().node_type = "yssbi.numeric.multiply".parse().unwrap();
    let value = wide
        .constants
        .values_mut()
        .find(|constant| {
            matches!(
                constant.data_value,
                yss_data_contract::DataValue::Float64(_)
            )
        })
        .unwrap();
    value.data_type =
        yss_data_contract::ValueType::Scalar(yss_data_contract::SemanticType::Numeric);
    value.data_value = yss_data_contract::DataValue::Int64(9_007_199_254_740_993);
    assert_eq!(
        execute(&wide, "yssbi.numeric.multiply").unwrap(),
        RuntimeValue::Integer(9_007_199_254_740_993)
    );
}

#[test]
fn constant_series_arithmetic_broadcasts_and_checks_lengths_and_divisors() {
    use yss_data_contract::{DataSeriesValue, DataValue, ValueType};
    let evaluate = |operator: &str,
                    element: ValueType,
                    values: &str,
                    scalar_type: ValueType,
                    scalar_value: DataValue,
                    scalar_left: bool| {
        let (mut document, operation) = division_graph(123);
        let node_type = format!("yssbi.numeric.{operator}");
        document.nodes.get_mut(&operation).unwrap().node_type = node_type.parse().unwrap();
        for constant in document.constants.values_mut() {
            if matches!(constant.data_value, DataValue::Float64(_)) {
                constant.data_type = ValueType::DataSeries(Box::new(element.clone()));
                constant.data_value = DataValue::DataSeries(DataSeriesValue::with_element_type(
                    values,
                    element.clone(),
                ));
            } else {
                constant.data_type = scalar_type.clone();
                constant.data_value = scalar_value.clone();
            }
            yss_graph_document::normalize_constant_value(constant).unwrap();
        }
        if scalar_left {
            let left = PortAddress::declared(operation, "left".parse().unwrap());
            let right = PortAddress::declared(operation, "right".parse().unwrap());
            for connection in document.connections.values_mut() {
                connection.input = if connection.input == left {
                    right.clone()
                } else {
                    left.clone()
                };
            }
        }
        execute(&document, &node_type)
    };
    for (operator, values, scalar, scalar_left, expected) in [
        ("power", r#"{"value":[1,2,3]}"#, 2, true, [2., 4., 8.]),
        ("power", r#"{"value":[1,2,3]}"#, 2, false, [1., 4., 9.]),
        ("log", r#"{"value":[2,4,8]}"#, 2, false, [1., 2., 3.]),
        ("log", r#"{"value":[2,4,16]}"#, 16, true, [4., 2., 1.]),
    ] {
        let numeric = ValueType::Scalar(yss_data_contract::SemanticType::Numeric);
        assert_eq!(
            evaluate(
                operator,
                numeric.clone(),
                values,
                numeric,
                DataValue::Int64(scalar),
                scalar_left
            )
            .unwrap(),
            RuntimeValue::List(expected.map(RuntimeValue::Decimal).into())
        );
    }
    assert_eq!(
        evaluate(
            "multiply",
            ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
            r#"{"value":[0.2,0.5,1.0]}"#,
            ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
            DataValue::Int64(123),
            false
        )
        .unwrap(),
        RuntimeValue::List([24.6, 61.5, 123.].map(RuntimeValue::Decimal).into())
    );
    assert_eq!(
        evaluate(
            "multiply",
            ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
            r#"{"value":[1,2,3]}"#,
            ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
            DataValue::Float64(0.5),
            false
        )
        .unwrap(),
        RuntimeValue::List([0.5, 1., 1.5].map(RuntimeValue::Decimal).into())
    );
    assert_eq!(
        evaluate(
            "subtract",
            ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
            r#"{"value":[1,2,3]}"#,
            ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
            DataValue::Float64(0.5),
            true
        )
        .unwrap(),
        RuntimeValue::List([-0.5, -1.5, -2.5].map(RuntimeValue::Decimal).into())
    );
    assert_eq!(
        evaluate(
            "multiply",
            ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
            r#"{"value":[1,2,3]}"#,
            ValueType::DataSeries(Box::new(ValueType::Scalar(
                yss_data_contract::SemanticType::Numeric
            ))),
            DataValue::DataSeries(DataSeriesValue::with_element_type(
                r#"{"value":[2]}"#,
                ValueType::Scalar(yss_data_contract::SemanticType::Numeric)
            )),
            false
        )
        .unwrap_err()
        .failure()
        .code,
        RunFailureCode::InvalidNumericInput
    );
    assert_eq!(
        evaluate(
            "divide",
            ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
            r#"{"value":[1,2,3]}"#,
            ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
            DataValue::Int64(0),
            false
        )
        .unwrap_err()
        .failure()
        .code,
        RunFailureCode::DivisionByZero
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
fn comparison_masks_feed_boolean_nodes_with_series_and_scalar_broadcasts() {
    use yss_data_contract::{DataSeriesValue, DataValue, SemanticType as S, ValueType as T};
    let mut document = GraphDocument::default();
    let [source, low, high, less, greater, gate, not, flag] =
        std::array::from_fn(|_| NodeId::new());
    for (id, kind) in [
        (source, "yssbi.constant.get"),
        (low, "yssbi.constant.get"),
        (high, "yssbi.constant.get"),
        (flag, "yssbi.constant.get"),
        (less, "yssbi.logic.less"),
        (greater, "yssbi.logic.greater"),
        (gate, "yssbi.logic.and"),
        (not, "yssbi.logic.not"),
    ] {
        document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: kind.parse().unwrap(),
                position: NodePosition { x: 0., y: 0. },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
    }
    set_constant(
        &mut document,
        source,
        T::DataSeries(Box::new(T::Scalar(S::Numeric))),
        DataValue::DataSeries(DataSeriesValue::with_element_type(
            r#"{"value":[1,2,3,null]}"#,
            T::Scalar(S::Numeric),
        )),
    );
    set_constant(
        &mut document,
        low,
        T::Scalar(S::Numeric),
        DataValue::Int64(1),
    );
    set_constant(
        &mut document,
        high,
        T::Scalar(S::Numeric),
        DataValue::Int64(3),
    );
    set_constant(
        &mut document,
        flag,
        T::Scalar(S::Binary),
        DataValue::Boolean(false),
    );
    for value in document.constants.values_mut() {
        yss_graph_document::normalize_constant_value(value).unwrap();
    }
    for (from, output, to, input) in [
        (source, "value", less, "left"),
        (source, "value", greater, "left"),
        (low, "value", greater, "right"),
        (high, "value", less, "right"),
        (less, "result", gate, "left"),
        (greater, "result", gate, "right"),
        (gate, "result", not, "input"),
    ] {
        let id = ConnectionId::new();
        document.connections.insert(
            id,
            DocumentConnection {
                id,
                output: PortAddress::declared(from, output.parse().unwrap()),
                input: PortAddress::declared(to, input.parse().unwrap()),
                order: None,
            },
        );
    }
    use RuntimeValue::{Bool as B, List as L, Null as N};
    assert_eq!(
        execute(&document, "yssbi.logic.not").unwrap(),
        L(Box::new([B(true), B(false), B(true), N]))
    );
    document.nodes.get_mut(&gate).unwrap().node_type = "yssbi.logic.or".parse().unwrap();
    assert_eq!(
        execute(&document, "yssbi.logic.not").unwrap(),
        L(Box::new([B(false), B(false), B(false), N]))
    );
    document.nodes.get_mut(&gate).unwrap().node_type = "yssbi.logic.and".parse().unwrap();
    let edge = document
        .connections
        .values_mut()
        .find(|edge| edge.output.node_id == greater && edge.input.node_id == gate)
        .unwrap();
    edge.output = PortAddress::declared(flag, "value".parse().unwrap());
    assert_eq!(
        execute(&document, "yssbi.logic.not").unwrap(),
        L(Box::new([B(true), B(true), B(true), B(true)]))
    );
}

#[test]
fn mathematical_constants_execute_without_project_constants_and_feed_arithmetic() {
    for (kind, expected, downstream, result) in [
        (
            "yssbi.constant.pi",
            std::f64::consts::PI,
            "yssbi.numeric.square",
            std::f64::consts::PI * std::f64::consts::PI,
        ),
        (
            "yssbi.constant.e",
            std::f64::consts::E,
            "yssbi.numeric.ln",
            1.,
        ),
    ] {
        let mut document = GraphDocument::default();
        let source = NodeId::new();
        document.nodes.insert(
            source,
            DocumentNode {
                id: source,
                node_type: kind.parse().unwrap(),
                position: NodePosition { x: 0., y: 0. },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
        assert_eq!(
            execute(&document, kind).unwrap(),
            RuntimeValue::Decimal(expected)
        );
        let target = NodeId::new();
        document.nodes.insert(
            target,
            DocumentNode {
                id: target,
                node_type: downstream.parse().unwrap(),
                position: NodePosition { x: 0., y: 0. },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
        let edge = ConnectionId::new();
        document.connections.insert(
            edge,
            DocumentConnection {
                id: edge,
                output: PortAddress::declared(source, "value".parse().unwrap()),
                input: PortAddress::declared(target, "input".parse().unwrap()),
                order: None,
            },
        );
        assert_eq!(
            execute(&document, downstream).unwrap(),
            RuntimeValue::Decimal(result)
        );
        assert!(document.constants.is_empty());
    }
}

#[test]
fn unary_arithmetic_nodes_execute_scalar_and_series_graphs() {
    use yss_data_contract::{DataSeriesValue, DataValue, SemanticType, ValueType};
    for (operation, values, expected) in [
        ("ln", [1., std::f64::consts::E], [0., 1.]),
        ("log2", [1., 8.], [0., 3.]),
        ("log10", [1., 100.], [0., 2.]),
        ("square", [-3., 0.], [9., 0.]),
        ("sqrt", [0., 9.], [0., 3.]),
    ] {
        let mut document = GraphDocument::default();
        let source = NodeId::new();
        let target = NodeId::new();
        let node_type = format!("yssbi.numeric.{operation}");
        for (id, kind) in [(source, "yssbi.constant.get"), (target, node_type.as_str())] {
            document.nodes.insert(
                id,
                DocumentNode {
                    id,
                    node_type: kind.parse().unwrap(),
                    position: NodePosition { x: 0., y: 0. },
                    parameters: ParameterValues::new(),
                    user_label: None,
                },
            );
        }
        let element = ValueType::Scalar(SemanticType::Numeric);
        set_constant(
            &mut document,
            source,
            element.clone(),
            DataValue::Float64(values[1]),
        );
        let edge = ConnectionId::new();
        document.connections.insert(
            edge,
            DocumentConnection {
                id: edge,
                output: PortAddress::declared(source, "value".parse().unwrap()),
                input: PortAddress::declared(target, "input".parse().unwrap()),
                order: None,
            },
        );
        assert_eq!(
            execute(&document, &node_type).unwrap(),
            RuntimeValue::Decimal(expected[1])
        );
        set_constant(
            &mut document,
            source,
            ValueType::DataSeries(Box::new(element.clone())),
            DataValue::DataSeries(DataSeriesValue::with_element_type(
                serde_json::json!({"value": values}).to_string(),
                element,
            )),
        );
        for value in document.constants.values_mut() {
            yss_graph_document::normalize_constant_value(value).unwrap();
        }
        assert_eq!(
            execute(&document, &node_type).unwrap(),
            RuntimeValue::List(expected.map(RuntimeValue::Decimal).into())
        );
    }
}

#[test]
fn unified_comparisons_prepare_broadcasts_and_reject_mismatched_meanings() {
    use yss_data_contract::{DataSeriesValue, DataValue, SemanticType as S, ValueType as T};
    let mut document = GraphDocument::default();
    let [left, right, comparison] = [NodeId::new(), NodeId::new(), NodeId::new()];
    for (id, kind) in [
        (left, "yssbi.constant.get"),
        (right, "yssbi.constant.get"),
        (comparison, "yssbi.logic.equal"),
    ] {
        document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: kind.parse().unwrap(),
                position: NodePosition { x: 0., y: 0. },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
    }
    set_constant(
        &mut document,
        left,
        T::DataSeries(Box::new(T::Scalar(S::Text))),
        DataValue::DataSeries(DataSeriesValue::with_element_type(
            r#"{"value":["001","1",null]}"#,
            T::Scalar(S::Text),
        )),
    );
    set_constant(
        &mut document,
        right,
        T::Scalar(S::Text),
        DataValue::String("1".into()),
    );
    for value in document.constants.values_mut() {
        yss_graph_document::normalize_constant_value(value).unwrap();
    }
    for (source, pin) in [(left, "left"), (right, "right")] {
        let id = ConnectionId::new();
        document.connections.insert(
            id,
            DocumentConnection {
                id,
                output: PortAddress::declared(source, "value".parse().unwrap()),
                input: PortAddress::declared(comparison, pin.parse().unwrap()),
                order: None,
            },
        );
    }
    assert_eq!(
        execute(&document, "yssbi.logic.equal").unwrap(),
        RuntimeValue::List(Box::new([
            RuntimeValue::Bool(false),
            RuntimeValue::Bool(true),
            RuntimeValue::Null
        ]))
    );
    for edge in document.connections.values_mut() {
        let pin = if edge.output.node_id == left {
            "right"
        } else {
            "left"
        };
        edge.input = PortAddress::declared(comparison, pin.parse().unwrap());
    }
    assert_eq!(
        execute(&document, "yssbi.logic.equal").unwrap(),
        RuntimeValue::List(Box::new([
            RuntimeValue::Bool(false),
            RuntimeValue::Bool(true),
            RuntimeValue::Null
        ]))
    );
    set_constant(
        &mut document,
        right,
        T::Scalar(S::Numeric),
        DataValue::Int64(1),
    );
    let graph = GraphResourcePath::new("events/comparison.yssbi-event").unwrap();
    let resources = ResourceCatalogSnapshot::new(
        BTreeMap::new(),
        BTreeMap::new(),
        ResourceCatalogFingerprint::from_bytes([0; 32]),
    );
    let analysis = analyze_document(&document, &graph, &resources);
    assert!(analysis.semantic_snapshot().has_blocking_diagnostics());
}

fn set_constant(
    document: &mut GraphDocument,
    node: NodeId,
    data_type: yss_data_contract::ValueType,
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

#[test]
fn semantic_conversion_executes_resolved_defaults_and_preserves_scalar_failures() {
    use yss_data_contract::{DataValue, SemanticType, ValueType};
    let mut document = GraphDocument::default();
    let source = NodeId::new();
    let convert = NodeId::new();
    for (id, kind) in [
        (source, "yssbi.constant.get"),
        (convert, "yssbi.value.convert"),
    ] {
        document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: kind.parse().unwrap(),
                position: NodePosition { x: 0., y: 0. },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
    }
    set_constant(
        &mut document,
        source,
        ValueType::Scalar(SemanticType::Text),
        DataValue::String("001".into()),
    );
    let id = ConnectionId::new();
    document.connections.insert(
        id,
        DocumentConnection {
            id,
            output: PortAddress::declared(source, "value".parse().unwrap()),
            input: PortAddress::declared(convert, "input".parse().unwrap()),
            order: None,
        },
    );
    document.nodes.get_mut(&convert).unwrap().parameters.insert(
        "target_type".parse().unwrap(),
        serde_json::json!("core.numeric"),
    );
    assert_eq!(
        execute(&document, "yssbi.value.convert").unwrap(),
        RuntimeValue::Decimal(1.)
    );
    document.nodes.get_mut(&convert).unwrap().parameters.insert(
        "numeric_mode".parse().unwrap(),
        serde_json::json!("integer"),
    );
    assert_eq!(
        execute(&document, "yssbi.value.convert").unwrap(),
        RuntimeValue::Integer(1)
    );
    set_constant(
        &mut document,
        source,
        ValueType::Scalar(SemanticType::Text),
        DataValue::String("9007199254740993".into()),
    );
    assert_eq!(
        execute(&document, "yssbi.value.convert").unwrap(),
        RuntimeValue::Integer(9_007_199_254_740_993)
    );
    document
        .nodes
        .get_mut(&convert)
        .unwrap()
        .parameters
        .insert("numeric_mode".parse().unwrap(), serde_json::json!("real"));
    assert!(execute(&document, "yssbi.value.convert").is_err());
    let element = ValueType::Scalar(SemanticType::Text);
    set_constant(
        &mut document,
        source,
        ValueType::DataSeries(Box::new(element.clone())),
        DataValue::DataSeries(yss_data_contract::DataSeriesValue::with_element_type(
            r#"{"value":["001",null,"002"]}"#,
            element,
        )),
    );
    for constant in document.constants.values_mut() {
        yss_graph_document::normalize_constant_value(constant).unwrap();
    }
    assert_eq!(
        execute(&document, "yssbi.value.convert").unwrap(),
        RuntimeValue::List(Box::new([
            RuntimeValue::Decimal(1.),
            RuntimeValue::Null,
            RuntimeValue::Decimal(2.)
        ]))
    );
}

#[test]
fn automatic_conversion_executes_the_downstream_target_without_rewriting_parameters() {
    use yss_data_contract::{DataValue, SemanticType, ValueType};
    let (mut document, divide) = division_graph(2);
    let source = document
        .connections
        .values()
        .find(|edge| edge.input == PortAddress::declared(divide, "left".parse().unwrap()))
        .unwrap()
        .output
        .node_id;
    set_constant(
        &mut document,
        source,
        ValueType::Scalar(SemanticType::Text),
        DataValue::String("12".into()),
    );
    let convert = NodeId::new();
    document.nodes.insert(
        convert,
        DocumentNode {
            id: convert,
            node_type: "yssbi.value.convert".parse().unwrap(),
            position: NodePosition { x: 0., y: 0. },
            parameters: ParameterValues::new(),
            user_label: None,
        },
    );
    for edge in document.connections.values_mut() {
        if edge.output.node_id == source {
            edge.output = PortAddress::declared(convert, "output".parse().unwrap());
        }
    }
    let id = ConnectionId::new();
    document.connections.insert(
        id,
        DocumentConnection {
            id,
            output: PortAddress::declared(source, "value".parse().unwrap()),
            input: PortAddress::declared(convert, "input".parse().unwrap()),
            order: None,
        },
    );
    assert_eq!(
        execute(&document, "yssbi.numeric.divide").unwrap(),
        RuntimeValue::Decimal(6.)
    );
    assert!(document.nodes[&convert].parameters.is_empty());
    let element = ValueType::Scalar(SemanticType::Text);
    set_constant(
        &mut document,
        source,
        ValueType::DataSeries(Box::new(element.clone())),
        DataValue::DataSeries(yss_data_contract::DataSeriesValue::with_element_type(
            r#"{"value":["12","4"]}"#,
            element,
        )),
    );
    for constant in document.constants.values_mut() {
        yss_graph_document::normalize_constant_value(constant).unwrap();
    }
    assert_eq!(
        execute(&document, "yssbi.numeric.divide").unwrap(),
        RuntimeValue::List(Box::new([
            RuntimeValue::Decimal(6.),
            RuntimeValue::Decimal(2.)
        ]))
    );
    assert!(document.nodes[&convert].parameters.is_empty());
}

#[test]
fn remaining_semantic_conversions_execute_automatically_and_keep_metadata_in_chains() {
    use yss_data_contract::{DataValue, SemanticType as S, ValueType};
    let evaluate =
        |input: &str, steps: Vec<serde_json::Value>, expected_semantic: S, expected: &str| {
            let mut document = GraphDocument::default();
            let mut add = |kind: &str| {
                let id = NodeId::new();
                document.nodes.insert(
                    id,
                    DocumentNode {
                        id,
                        node_type: kind.parse().unwrap(),
                        position: NodePosition { x: 0., y: 0. },
                        parameters: ParameterValues::new(),
                        user_label: None,
                    },
                );
                id
            };
            let source = add("yssbi.constant.get");
            let expected_node = add("yssbi.constant.get");
            let equal = add("yssbi.logic.equal");
            let expected_ordinal =
                (expected_semantic == S::Ordinal).then(|| add("yssbi.value.convert"));
            let conversions = steps
                .iter()
                .map(|_| add("yssbi.value.convert"))
                .collect::<Vec<_>>();
            set_constant(
                &mut document,
                source,
                ValueType::Scalar(S::Text),
                DataValue::String(input.into()),
            );
            set_constant(
                &mut document,
                expected_node,
                ValueType::Scalar(if expected_ordinal.is_some() {
                    S::Text
                } else {
                    expected_semantic
                }),
                DataValue::String(expected.into()),
            );
            if let Some(id) = expected_ordinal {
                document.nodes.get_mut(&id).unwrap().parameters = ParameterValues::from([
                    (
                        "target_type".parse().unwrap(),
                        serde_json::json!("core.ordinal"),
                    ),
                    (
                        "semantic_domain".parse().unwrap(),
                        steps[0]["semantic_domain"].clone(),
                    ),
                ]);
                let edge = ConnectionId::new();
                document.connections.insert(
                    edge,
                    DocumentConnection {
                        id: edge,
                        output: PortAddress::declared(expected_node, "value".parse().unwrap()),
                        input: PortAddress::declared(id, "input".parse().unwrap()),
                        order: None,
                    },
                );
            }
            let mut previous = (source, "value");
            for (id, parameters) in conversions.into_iter().zip(steps) {
                document.nodes.get_mut(&id).unwrap().parameters = parameters
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(key, value)| (key.parse().unwrap(), value.clone()))
                    .collect();
                let edge = ConnectionId::new();
                document.connections.insert(
                    edge,
                    DocumentConnection {
                        id: edge,
                        output: PortAddress::declared(previous.0, previous.1.parse().unwrap()),
                        input: PortAddress::declared(id, "input".parse().unwrap()),
                        order: None,
                    },
                );
                previous = (id, "output");
            }
            for (output, port, input) in [
                (previous.0, previous.1, "left"),
                (
                    expected_ordinal.unwrap_or(expected_node),
                    if expected_ordinal.is_some() {
                        "output"
                    } else {
                        "value"
                    },
                    "right",
                ),
            ] {
                let edge = ConnectionId::new();
                document.connections.insert(
                    edge,
                    DocumentConnection {
                        id: edge,
                        output: PortAddress::declared(output, port.parse().unwrap()),
                        input: PortAddress::declared(equal, input.parse().unwrap()),
                        order: None,
                    },
                );
            }
            assert_eq!(
                execute(&document, "yssbi.logic.equal").unwrap(),
                RuntimeValue::Bool(true)
            );
            for constant in document.constants.values_mut() {
                let DataValue::String(value) = &constant.data_value else {
                    panic!("string fixture");
                };
                let encoded = serde_json::json!({"value":[value, null]}).to_string();
                let element = constant.data_type.clone();
                constant.data_type = ValueType::DataSeries(Box::new(element.clone()));
                constant.data_value = DataValue::DataSeries(
                    yss_data_contract::DataSeriesValue::with_element_type(encoded, element),
                );
                yss_graph_document::normalize_constant_value(constant).unwrap();
            }
            assert_eq!(
                execute(&document, "yssbi.logic.equal").unwrap(),
                RuntimeValue::List(Box::new([RuntimeValue::Bool(true), RuntimeValue::Null]))
            );
        };
    let domain = serde_json::json!({"values":[{"value":"002","label":"low"},{"value":"001","label":"high"}]});
    for semantic in [S::Categorical, S::Ordinal, S::Identifier] {
        evaluate(
            "001",
            vec![serde_json::json!({"semantic_domain":domain})],
            semantic,
            "001",
        );
    }
    evaluate(
        "001",
        vec![
            serde_json::json!({"target_type":"core.ordinal","semantic_domain":domain}),
            serde_json::json!({"target_type":"core.categorical"}),
            serde_json::json!({"target_type":"core.identifier"}),
            serde_json::json!({"target_type":"core.text"}),
        ],
        S::Text,
        "001",
    );
    evaluate(
        "17/09/2026 08:30:00 +0800",
        vec![serde_json::json!({"datetime_format":"%d/%m/%Y %H:%M:%S %z"})],
        S::Datetime,
        "2026-09-17T08:30:00",
    );
    evaluate(
        "17/09/2026 08:30:00 +0800",
        vec![
            serde_json::json!({"target_type":"core.datetime","datetime_format":"%d/%m/%Y %H:%M:%S %z"}),
            serde_json::json!({"target_type":"core.text"}),
        ],
        S::Text,
        "2026-09-17T08:30:00",
    );
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
        (fit, "yssbi.statistics.linear.fit", vec![]),
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
fn decompose_returns_lazy_typed_columns_before_the_data_file_exists() {
    use arrow::array::{BooleanArray, Int64Array, StringArray};
    use arrow::datatypes::{DataType, Field, Schema};
    use arrow::record_batch::RecordBatch;
    use yss_database_contract::DatabaseId;
    use yss_graph_execution::plan::{
        PlanResourceId, PlanResourceObservedState, PlanResourceRequirement, PlanResourceVersion,
        ResourceAccess, ResourceKind,
    };
    use yss_graph_execution::resource_preparation::RunResourceBinding;
    use yss_graph_resource_contract::{ColumnSchema, DataSchema, GraphResourceId};
    use yss_relational_contract::{RelationBinding, RelationControl};

    let source = NodeId::new();
    let decompose = NodeId::new();
    let mut document = GraphDocument::default();
    for (id, kind, parameters) in [
        (
            source,
            "yssbi.dataframe.source.get",
            ParameterValues::from([("dataframe".parse().unwrap(), serde_json::json!("data"))]),
        ),
        (
            decompose,
            "yssbi.dataframe.decompose",
            ParameterValues::new(),
        ),
    ] {
        document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: kind.parse().unwrap(),
                position: NodePosition { x: 0., y: 0. },
                parameters,
                user_label: None,
            },
        );
    }
    let id = ConnectionId::new();
    document.connections.insert(
        id,
        DocumentConnection {
            id,
            output: PortAddress::declared(source, "dataframe".parse().unwrap()),
            input: PortAddress::declared(decompose, "dataframe".parse().unwrap()),
            order: None,
        },
    );
    let resources = ResourceCatalogSnapshot::new(
        BTreeMap::new(),
        BTreeMap::from([(
            GraphResourceId::new("data"),
            DataSchema {
                columns: [
                    (
                        "count",
                        yss_data_contract::ValueType::Scalar(
                            yss_data_contract::SemanticType::Numeric,
                        ),
                    ),
                    (
                        "label.列",
                        yss_data_contract::ValueType::Scalar(yss_data_contract::SemanticType::Text),
                    ),
                    (
                        "flag",
                        yss_data_contract::ValueType::Scalar(
                            yss_data_contract::SemanticType::Binary,
                        ),
                    ),
                ]
                .into_iter()
                .map(|(name, data_type)| ColumnSchema {
                    semantic: None,
                    physical_type: None,
                    name: name.into(),
                    data_type,
                })
                .collect(),
            },
        )]),
        ResourceCatalogFingerprint::from_bytes([0; 32]),
    );
    let graph = GraphResourcePath::new("events/decompose.yssbi-event").unwrap();
    let analysis = analyze_document(&document, &graph, &resources);
    let runtime = ExecutionRuntimeState::new(
        ExecutionSessionId::new(uuid::Uuid::new_v4()),
        RuntimeGeneration::INITIAL,
        yss_node_kernel::KernelRegistry::default().into(),
    );
    let resource = PlanResourceId::from_existing("data".into());
    let version = PlanResourceVersion::from_existing("7".into());
    let session = PlanProjectSessionId::from_existing("decompose-session".into());
    let package = runtime
        .prepare_graph_package(
            &graph,
            &analysis,
            PlanBasis::new(
                session.clone(),
                PlanRegistryFingerprint::from_bytes([0; 32]),
                yss_node_kernel::KernelRegistry::default().fingerprint(),
                BTreeMap::from([(resource.clone(), version.clone())]),
                BTreeMap::from([(
                    resource.clone(),
                    PlanResourceObservedState::Present(version.clone()),
                )]),
            ),
        )
        .unwrap();
    let outputs = package
        .plan()
        .operations()
        .iter()
        .find(|operation| operation.node_type().as_str() == "yssbi.dataframe.decompose")
        .unwrap()
        .outputs()
        .to_vec();
    assert_eq!(outputs.len(), 3);
    let plan = runtime
        .prepare_package(package, RuntimeGeneration::INITIAL)
        .unwrap();
    struct DatasetLease(std::path::PathBuf);
    impl Drop for DatasetLease {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let lease = Arc::new(DatasetLease(
        std::env::temp_dir().join(format!("yss-decompose-{}", uuid::Uuid::new_v4())),
    ));
    let path = lease.0.join("part.parquet");
    let schema = Arc::new(
        yss_tabular_arrow::with_row_columns(
            Schema::new(vec![
                Field::new("count", DataType::Int64, true),
                Field::new("label.列", DataType::Utf8, true),
                Field::new("flag", DataType::Boolean, true),
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
                project_session: "decompose-session".into(),
                dataset: DatabaseId::from_existing("data".into()),
                snapshot: "generation-7".into(),
                revision: 7,
            },
            schema.clone(),
            std::slice::from_ref(&path),
            lease,
        )
        .unwrap();
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
                    RuntimeValue::Relation(relation.clone()),
                )],
            ),
            &ResourceProviderFactory::new("decompose-session".into()),
            &RunExecutionControl::with_cancellation(
                Arc::new(AtomicBool::new(false)),
                Instant::now() + Duration::from_secs(30),
            ),
            &PlanExecutionDemand::Outputs {
                outputs: outputs
                    .iter()
                    .map(|output| output.output().clone())
                    .collect(),
                include_default_results: false,
            },
            None,
            |_| {},
        )
        .unwrap();
    assert!(
        !path.exists(),
        "neither plan preparation nor Decompose may scan the source"
    );
    let columns = outputs
        .iter()
        .map(|output| {
            let result = executed
                .handoff()
                .results()
                .iter()
                .find(|result| result.output() == output.output())
                .unwrap();
            let RuntimeValue::Series(column) = result.value().value() else {
                panic!("lazy column output");
            };
            assert_eq!(column.relation(), &relation);
            assert_eq!(
                column.column(),
                output.contract().schema.as_ref().unwrap()[0].name.as_ref()
            );
            column.clone()
        })
        .collect::<Vec<_>>();
    let input = RuntimeValue::Series(
        columns
            .iter()
            .find(|column| column.column() == "count")
            .unwrap()
            .clone(),
    );
    let converted = runtime
        .kernels()
        .execute(
            &yss_node_kernel::KernelId::new("yssbi.value.convert".into()).unwrap(),
            &yss_node_kernel::KernelInvocation {
                inputs: &[input],
                input_templates: &[None],
                parameters: BTreeMap::from([
                    (
                        yss_node_kernel::KernelParameterKey::new("target_type".into()).unwrap(),
                        std::borrow::Cow::Owned(RuntimeValue::String("auto".into())),
                    ),
                    (
                        yss_node_kernel::KernelParameterKey::new("numeric_mode".into()).unwrap(),
                        std::borrow::Cow::Owned(RuntimeValue::String("real".into())),
                    ),
                    (
                        yss_node_kernel::KernelParameterKey::new("semantic_domain".into()).unwrap(),
                        std::borrow::Cow::Owned(RuntimeValue::Record(BTreeMap::new())),
                    ),
                    (
                        yss_node_kernel::KernelParameterKey::new("datetime_kind".into()).unwrap(),
                        std::borrow::Cow::Owned(RuntimeValue::String("auto".into())),
                    ),
                    (
                        yss_node_kernel::KernelParameterKey::new("datetime_precision".into())
                            .unwrap(),
                        std::borrow::Cow::Owned(RuntimeValue::String("microseconds".into())),
                    ),
                    (
                        yss_node_kernel::KernelParameterKey::new("datetime_format".into()).unwrap(),
                        std::borrow::Cow::Owned(RuntimeValue::String("".into())),
                    ),
                ]),
                outputs: &[yss_node_kernel::KernelOutputSpec {
                    data_type: yss_data_contract::ValueType::DataSeries(Box::new(
                        yss_data_contract::ValueType::Scalar(
                            yss_data_contract::SemanticType::Numeric,
                        ),
                    )),
                    fields: None,
                }],
                control: &yss_node_kernel::KernelControl::new(
                    Arc::new(AtomicBool::new(false)),
                    Instant::now() + Duration::from_secs(30),
                ),
            },
        )
        .unwrap();
    let RuntimeValue::Series(converted) = &converted[0] else {
        panic!("conversion must remain lazy");
    };
    assert_eq!(converted.relation(), &relation);
    assert!(!path.exists(), "conversion kernel must not scan the source");
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(Int64Array::from(vec![Some(2), None, Some(1)])),
            Arc::new(StringArray::from(vec![Some("b"), None, Some("a")])),
            Arc::new(BooleanArray::from(vec![Some(false), None, Some(true)])),
            Arc::new(Int64Array::from(vec![12, 13, 11])),
            Arc::new(StringArray::from(vec!["2", "3", "1"])),
        ],
    )
    .unwrap();
    yss_tabular_io::write_parquet_batches(&path, schema, [Ok(batch)]).unwrap();
    let control = RelationControl {
        cancellation: Arc::new(AtomicBool::new(false)),
        deadline: Instant::now() + Duration::from_secs(30),
        max_input_bytes: 1024 * 1024,
    };
    let converted_page = converted
        .as_relation()
        .unwrap()
        .page(0, 4, &control)
        .unwrap();
    assert_eq!(converted.plan().field().data_type(), &DataType::Float64);
    assert_eq!(
        serde_json::to_value(converted_page.data.columns()[0].values()).unwrap(),
        serde_json::json!([1.0, 2.0, null])
    );
    for column in columns {
        let projected = column.as_relation().unwrap();
        let page = projected.page(0, 4, &control).unwrap();
        let expected = match column.column() {
            "count" => serde_json::json!([1, 2, null]),
            "label.列" => serde_json::json!(["a", "b", null]),
            "flag" => serde_json::json!([true, false, null]),
            _ => panic!("unexpected column"),
        };
        assert_eq!(page.columns.len(), 1);
        assert_eq!(page.columns[0].name.as_ref(), column.column());
        assert_eq!(
            serde_json::to_value(page.data.columns()[0].values()).unwrap(),
            expected
        );
    }
}

#[test]
fn project_dataset_graph_runs_through_application_authority_and_paged_results() {
    use yss_application::graph::results::ResultPinQuery;
    use yss_application::graph::run::{RunGraphRequest, run_graph};
    use yss_application::session::{
        ApplicationSessionEpoch, ApplicationSessionSlot, ApplicationState,
    };
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

    let candidate = yss_application::session::build_current_project_candidate(
        ApplicationSessionEpoch::INITIAL,
        project.clone(),
        [],
        &yss_application::session::NodeComponents::builtins().unwrap(),
    )
    .unwrap();
    let app = ApplicationState::new(Arc::new(ApplicationSessionSlot::new(
        yss_application::session::NodeComponents::builtins().unwrap(),
    )));
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
    let (mut document, [_, _, filter, response, predictor, fit]) = relational_document(&resource);
    document.nodes.remove(&response);
    document.nodes.remove(&predictor);
    document.connections.retain(|_, connection| {
        ![response, predictor].contains(&connection.input.node_id)
            && ![response, predictor].contains(&connection.output.node_id)
    });
    let rename = NodeId::new();
    let decompose = NodeId::new();
    for (id, kind, parameters) in [
        (
            rename,
            "yssbi.dataframe.rename",
            vec![
                ("from", serde_json::json!("x")),
                ("to", serde_json::json!("predictor.value")),
            ],
        ),
        (decompose, "yssbi.dataframe.decompose", vec![]),
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
    for (output_node, output_key, input_node, input_key) in [
        (filter, "result", rename, "source"),
        (rename, "result", decompose, "dataframe"),
    ] {
        let id = ConnectionId::new();
        document.connections.insert(
            id,
            DocumentConnection {
                id,
                output: PortAddress::declared(output_node, output_key.parse().unwrap()),
                input: PortAddress::declared(input_node, input_key.parse().unwrap()),
                order: None,
            },
        );
    }
    let path = GraphResourcePath::new("events/relational.yssbi-event").unwrap();
    let file = GraphResourceFile {
        kind: yss_graph_document::GraphResourceKind::Event,
        name: "Relational".into(),
        document: document.clone(),
        function: None,
    };
    std::fs::write(root.join(path.as_str()), serde_json::to_vec(&file).unwrap()).unwrap();
    project.load_graph_document(&instance, &path, 1).unwrap();
    let projection = app
        .resolve_graph_document(
            instance.clone(),
            path.clone(),
            document.clone(),
            "en".into(),
        )
        .unwrap();
    let columns = projection
        .nodes
        .iter()
        .find(|node| node.node_id == decompose)
        .unwrap();
    let column = |name: &str| {
        columns
            .ports
            .iter()
            .find(|port| port.display.label.as_ref() == name)
            .unwrap()
            .address
            .clone()
    };
    let x_output = column("predictor.value");
    let y_output = column("y");
    let predictor_input = document
        .port_bindings
        .keys()
        .find(|port| port.node_id == fit)
        .unwrap()
        .clone();
    let scale = NodeId::new();
    let multiply = NodeId::new();
    let half = NodeId::new();
    let factor = NodeId::new();
    for (id, node_type) in [
        (scale, "yssbi.numeric.multiply"),
        (multiply, "yssbi.numeric.multiply"),
        (half, "yssbi.constant.get"),
        (factor, "yssbi.constant.get"),
    ] {
        document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: node_type.parse().unwrap(),
                position: NodePosition { x: 0., y: 0. },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
    }
    set_constant(
        &mut document,
        half,
        yss_data_contract::ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
        yss_data_contract::DataValue::Float64(0.5),
    );
    set_constant(
        &mut document,
        factor,
        yss_data_contract::ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
        yss_data_contract::DataValue::Int64(123),
    );
    let fixture_capture = project
        .capture_graph_overwrite_operation(&instance, &path, OperationId::new())
        .unwrap();
    project
        .commit_graph_candidate(fixture_capture.into_authority(), Arc::new(document.clone()))
        .unwrap();
    for (output, input) in [
        (
            x_output.clone(),
            PortAddress::declared(scale, "left".parse().unwrap()),
        ),
        (
            PortAddress::declared(half, "value".parse().unwrap()),
            PortAddress::declared(scale, "right".parse().unwrap()),
        ),
        (
            PortAddress::declared(scale, "result".parse().unwrap()),
            PortAddress::declared(multiply, "left".parse().unwrap()),
        ),
        (
            PortAddress::declared(factor, "value".parse().unwrap()),
            PortAddress::declared(multiply, "right".parse().unwrap()),
        ),
        (
            PortAddress::declared(multiply, "result".parse().unwrap()),
            predictor_input,
        ),
        (
            y_output.clone(),
            PortAddress::declared(fit, "response".parse().unwrap()),
        ),
    ] {
        let version = project
            .read_graph_editing(&instance, &path)
            .unwrap()
            .state
            .version;
        document = app
            .edit_graph(
                yss_application::graph::editing::GraphEditRequest {
                    project_instance_id: instance.clone(),
                    graph_path: path.clone(),
                    version,
                    operation_id: OperationId::new(),
                    locale: "en".into(),
                },
                yss_graph_editor::EditorGraphMutation::Connect {
                    output,
                    input,
                    order: None,
                },
            )
            .unwrap()
            .update
            .document;
    }
    assert!(document.port_bindings.contains_key(&x_output));
    assert!(document.port_bindings.contains_key(&y_output));
    // A saved column address must still select the current name after an upstream rename.
    document
        .nodes
        .get_mut(&rename)
        .unwrap()
        .parameters
        .insert("to".parse().unwrap(), serde_json::json!("预测.value"));
    let [assemble, rows, columns, join, first_column, second_column] =
        std::array::from_fn(|_| NodeId::new());
    for (id, kind, parameters) in [
        (assemble, "yssbi.dataframe.combine", vec![]),
        (rows, "yssbi.dataframe.concat.rows", vec![]),
        (columns, "yssbi.dataframe.concat.columns", vec![]),
        (
            join,
            "yssbi.dataframe.join",
            vec![
                ("left_keys", serde_json::json!(["x"])),
                ("right_keys", serde_json::json!(["x"])),
            ],
        ),
        (
            first_column,
            "yssbi.dataframe.project",
            vec![("columns", serde_json::json!(["预测.value"]))],
        ),
        (
            second_column,
            "yssbi.dataframe.project",
            vec![("columns", serde_json::json!(["y"]))],
        ),
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
    for (target, template, outputs) in [
        (assemble, "series", vec![x_output.clone(), y_output.clone()]),
        (
            rows,
            "frames",
            vec![PortAddress::declared(filter, "result".parse().unwrap()); 2],
        ),
        (
            columns,
            "frames",
            vec![
                PortAddress::declared(first_column, "result".parse().unwrap()),
                PortAddress::declared(second_column, "result".parse().unwrap()),
            ],
        ),
    ] {
        for (index, output) in outputs.into_iter().enumerate() {
            let input = PortAddress::instance(
                target,
                template.parse().unwrap(),
                yss_graph_document::PortInstanceId::new(),
            );
            document.port_bindings.insert(
                input.clone(),
                yss_graph_document::DynamicPortBinding::UserCreated {
                    order: yss_graph_document::OrderKey::new(index.to_string()),
                },
            );
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
    }
    for (from, target, key) in [
        (rename, first_column, "source"),
        (rename, second_column, "source"),
        (filter, join, "left"),
        (filter, join, "right"),
    ] {
        let id = ConnectionId::new();
        document.connections.insert(
            id,
            DocumentConnection {
                id,
                output: PortAddress::declared(from, "result".parse().unwrap()),
                input: PortAddress::declared(target, key.parse().unwrap()),
                order: None,
            },
        );
    }
    let document: GraphDocument =
        serde_json::from_slice(&serde_json::to_vec(&document).unwrap()).unwrap();
    let fixture_capture = project
        .capture_graph_overwrite_operation(&instance, &path, OperationId::new())
        .unwrap();
    project
        .commit_graph_candidate(fixture_capture.into_authority(), Arc::new(document.clone()))
        .unwrap();
    let ready = app
        .open_graph(yss_application::graph::open::OpenGraphRequest::new(
            instance.clone(),
            path.clone(),
            0,
            "en",
        ))
        .unwrap()
        .projection()
        .clone();
    let run_id = run_graph(
        &app,
        RunGraphRequest::new(
            instance,
            path.clone(),
            document,
            ready.basis.semantic_input_hash,
        ),
    )
    .unwrap();
    let query = |node, key: &str| {
        ResultPinQuery::new(
            path.clone(),
            PortAddress::declared(node, key.parse().unwrap()),
        )
    };
    let result = app.query_pin_result(query(fit, "fitted")).unwrap().unwrap();
    for (node, output, count, names) in [
        (assemble, "dataframe", 4, vec!["预测.value", "y"]),
        (rows, "result", 8, vec!["x", "y"]),
        (columns, "result", 4, vec!["预测.value", "y"]),
        (join, "result", 4, vec!["x", "y", "x_right", "y_right"]),
    ] {
        let result = app.query_pin_result(query(node, output)).unwrap().unwrap();
        assert!(matches!(result.value().value(), RuntimeValue::Relation(_)));
        let page = app
            .query_result_page(result.provenance().reference(), 0, 20)
            .unwrap()
            .unwrap();
        assert_eq!(page.values.len(), count);
        assert_eq!(
            page.columns
                .iter()
                .map(|column| column.name.as_ref())
                .collect::<Vec<_>>(),
            names
        );
    }
    assert_eq!(result.provenance().run_id(), run_id);
    let RuntimeValue::List(fitted) = result.value().value() else {
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
    let RuntimeValue::List(residuals) = result.value().value() else {
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
        RuntimeValue::Relation(_)
    ));
    let page = app
        .query_result_page(relation.provenance().reference(), 0, 2)
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
    let mut series = Vec::new();
    for (output, name, expected) in [(x_output, "预测.value", [3, 4]), (y_output, "y", [7, 9])] {
        let result = app
            .query_pin_result(ResultPinQuery::new(path.clone(), output))
            .unwrap()
            .unwrap();
        let RuntimeValue::Series(column) = result.value().value() else {
            panic!("Decompose must retain a lazy column handle");
        };
        assert_eq!(column.column(), name);
        series.push(column.clone());
        let page = app
            .query_result_page(result.provenance().reference(), 0, 2)
            .unwrap()
            .unwrap();
        assert_eq!(page.columns.len(), 1);
        assert_eq!(page.columns[0].name.as_ref(), name);
        assert_eq!(
            page.values.as_ref(),
            expected.map(|value| RuntimeValue::List(Box::new([RuntimeValue::Unsigned(value)])))
        );
        assert!(page.has_more);
    }
    assert_eq!(series[0].relation(), series[1].relation());
    let result = app
        .query_pin_result(query(multiply, "result"))
        .unwrap()
        .unwrap();
    let RuntimeValue::Series(computed) = result.value().value() else {
        panic!("arithmetic must retain a lazy series")
    };
    assert_eq!(computed.relation(), series[0].relation());
    let page = app
        .query_result_page(result.provenance().reference(), 0, 2)
        .unwrap()
        .unwrap();
    assert_eq!(page.columns.len(), 1);
    assert_eq!(page.columns[0].data_type.as_ref(), "Float64");
    assert!(page.has_more);
    for (row, expected) in page.values.iter().zip([184.5, 246.]) {
        let RuntimeValue::List(values) = row else {
            panic!("one-column row")
        };
        let actual = match values[0] {
            RuntimeValue::Decimal(value) => value,
            RuntimeValue::Unsigned(value) => value as f64,
            _ => panic!("numeric result"),
        };
        assert!((actual - expected).abs() < 1e-10);
    }
}
