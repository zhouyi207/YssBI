use super::*;
use crate::{GraphDiagnosticLocation, GraphSchemaIssue};
use yss_data_contract::{SemanticType, ValueType};
use yss_graph_document::{
    ConnectionId, DocumentConnection, DocumentNode, DynamicMemberLocator, DynamicPortBinding,
    GraphDocument, LastKnownPortMetadata, NodeId, NodePosition, OrderKey, ParameterValues,
    PortInstanceId,
};
use yss_graph_resource_contract::{
    FunctionCatalogEntry, FunctionParameterContract, FunctionSignature,
};
use yss_node_protocol::{
    RelationalScalarType, ResolvedSchemaFact, SchemaColumnRef, SchemaExpr, SchemaField,
};

#[test]
fn dataframe_arguments_specialize_independently_and_missing_columns_keep_inner_locations() {
    let registry = yss_node_catalog::build_builtin_node_system()
        .unwrap()
        .registry;
    let function = GraphResourcePath::new("functions/Select.yssbi-function").unwrap();
    let parameter = FunctionParameterId::new("frame");
    let entry = NodeId::new();
    let select = NodeId::new();
    let exit = NodeId::new();
    let mut body = GraphDocument::default();
    for (id, kind, key, value) in [
        (
            entry,
            "yssbi.project.function.entry",
            "function",
            serde_json::json!(function.as_str()),
        ),
        (
            select,
            "yssbi.dataframe.project",
            "columns",
            serde_json::json!(["sales"]),
        ),
        (
            exit,
            "yssbi.project.function.return",
            "function",
            serde_json::json!(function.as_str()),
        ),
    ] {
        body.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: kind.parse().unwrap(),
                position: NodePosition { x: 0., y: 0. },
                parameters: ParameterValues::from([(key.parse().unwrap(), value)]),
                user_label: None,
            },
        );
    }
    let argument =
        PortAddress::instance(entry, "parameters".parse().unwrap(), PortInstanceId::new());
    let returned = PortAddress::instance(exit, "results".parse().unwrap(), PortInstanceId::new());
    for (address, member) in [
        (&argument, parameter.clone()),
        (&returned, FunctionParameterId::new("return")),
    ] {
        body.port_bindings.insert(
            address.clone(),
            DynamicPortBinding::Resolved {
                origin: DynamicMemberLocator::FunctionParameter {
                    function: function.clone(),
                    parameter: member,
                },
                order: OrderKey::new("0"),
                last_known: LastKnownPortMetadata::default(),
            },
        );
    }
    for (output, input) in [
        (
            argument,
            PortAddress::declared(select, "source".parse().unwrap()),
        ),
        (
            PortAddress::declared(select, "result".parse().unwrap()),
            returned.clone(),
        ),
    ] {
        let id = ConnectionId::new();
        body.connections.insert(
            id,
            DocumentConnection {
                id,
                output,
                input,
                order: None,
            },
        );
    }
    let resources = ResourceCatalogSnapshot::new(
        BTreeMap::from([(
            function.clone(),
            FunctionCatalogEntry::new(FunctionSignature::new(
                vec![FunctionParameterContract::new(
                    parameter.clone(),
                    "Data",
                    ValueType::DataFrame,
                )],
                Some(ValueType::DataFrame),
            )),
        )]),
        BTreeMap::new(),
    )
    .with_function_document(&function, body.clone());
    let mut root = GraphDocument::default();
    let call = NodeId::new();
    root.nodes.insert(
        call,
        DocumentNode {
            id: call,
            node_type: "yssbi.project.function.call".parse().unwrap(),
            position: NodePosition { x: 0., y: 0. },
            user_label: None,
            parameters: ParameterValues::from([(
                "target".parse().unwrap(),
                serde_json::json!(function.as_str()),
            )]),
        },
    );
    let functions = crate::function_validation::resolve(&root, &registry, &resources);
    let definition = &functions.functions[&function];
    assert_eq!(definition.state, GraphFunctionState::Unbound);
    assert!(
        functions.diagnostics.is_empty(),
        "{:?}",
        functions.diagnostics
    );
    assert!(
        definition.semantics.ready().is_none(),
        "a definition has no concrete dataframe argument"
    );
    let value = |name: &str, semantic| {
        let fields = vec![SchemaField {
            name: SchemaColumnRef(name.into()),
            scalar_type: RelationalScalarType::Known(semantic),
            lineage: None,
        }];
        GraphFunctionArgument {
            value_type: ResolvedType::Nominal("tabular.dataframe".parse().unwrap()),
            schema: GraphSchemaState::Exact(ResolvedSchemaFact {
                expression: SchemaExpr::Fixed {
                    fields: fields.clone(),
                },
                fields,
            }),
        }
    };
    let mut cache = GraphSemanticCache::default();
    let bind = |argument, cache: &mut GraphSemanticCache| {
        specialize_function(
            &function,
            definition,
            &registry,
            &resources,
            &BTreeMap::from([(parameter.clone(), argument)]),
            &BTreeMap::new(),
            cache,
        )
        .unwrap()
    };
    let numeric = bind(value("sales", SemanticType::Numeric), &mut cache);
    assert_eq!(numeric.state, GraphFunctionState::Ready);
    assert!(
        numeric.semantics.ready().is_some(),
        "{:?}",
        numeric.semantics.diagnostics()
    );
    let text = bind(value("sales", SemanticType::Text), &mut cache);
    assert!(text.semantics.ready().is_some());
    let result_type = |function: &GraphFunctionSemanticFact| {
        function
            .semantics
            .concrete_interface()
            .port(&returned)
            .unwrap()
            .schema_state
            .exact()
            .unwrap()
            .fields[0]
            .scalar_type
    };
    assert_eq!(
        result_type(&numeric),
        RelationalScalarType::Known(SemanticType::Numeric)
    );
    assert_eq!(
        result_type(&text),
        RelationalScalarType::Known(SemanticType::Text)
    );
    let missing = bind(value("income", SemanticType::Numeric), &mut cache);
    assert_eq!(missing.state, GraphFunctionState::Invalid);
    assert!(missing.semantics.ready().is_none());
    assert!(
        missing
            .semantics
            .node(select)
            .unwrap()
            .ports
            .iter()
            .any(|port| port.schema_state.issue() == Some(GraphSchemaIssue::MissingColumn))
    );
    assert!(missing.semantics.diagnostics().iter().any(|diagnostic| diagnostic.blocking
        && matches!(&diagnostic.primary, GraphDiagnosticLocation::Port(port) if port.node_id == select)));
    assert_eq!(
        bind(value("sales", SemanticType::Numeric), &mut cache),
        numeric,
        "cached calls are isolated by argument facts"
    );
    assert!(matches!(
        specialize_function(
            &function,
            definition,
            &registry,
            &resources,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &mut cache
        ),
        Err(GraphFunctionBindingError::ArgumentSetMismatch)
    ));
    let mut wrong = value("sales", SemanticType::Numeric);
    wrong.value_type = ResolvedType::Nominal("core.text".parse().unwrap());
    assert!(matches!(
        specialize_function(
            &function,
            definition,
            &registry,
            &resources,
            &BTreeMap::from([(parameter, wrong)]),
            &BTreeMap::new(),
            &mut cache
        ),
        Err(GraphFunctionBindingError::ArgumentTypeMismatch { .. })
    ));
    assert_eq!(resources.function_document(&function), Some(&body));
    assert!(
        definition.semantics.ready().is_none(),
        "specialization never overwrites the shared definition"
    );
    let unavailable = crate::resolve_graph_semantics(&root, &registry, &resources)
        .with_execution_kernel_support(&|kernel| kernel != "yssbi.dataframe.project");
    assert_eq!(
        unavailable.functions()[&function].state,
        GraphFunctionState::Invalid
    );
    assert!(
        unavailable.functions()[&function]
            .semantics
            .diagnostics()
            .iter()
            .any(
                |diagnostic| diagnostic.code.as_str() == "graph.node.kernel_unavailable"
                    && diagnostic.primary == GraphDiagnosticLocation::Node(select)
            )
    );
    assert!(
        !unavailable
            .diagnostics()
            .iter()
            .any(
                |diagnostic| diagnostic.code.as_str() == "graph.node.kernel_unavailable"
                    && diagnostic.primary == GraphDiagnosticLocation::Node(call)
            ),
        "function dispatch is not a missing leaf kernel"
    );
}
