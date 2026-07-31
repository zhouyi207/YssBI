use super::{LEGACY_NODE_IDS, NODES, build_provider_fragment};
use crate::node_system::catalog::builtin::build_builtin_registry;
use crate::node_system::compiler::{
    CompileCancellationToken, LoweredKernel, LoweringContext, NodeImplementation,
};
use crate::node_system::document::{NodeId, PortAddress};
use crate::node_system::plan::{RelationalOperator, RelationalOperatorIndex, ResourceId, ValueRef};
use crate::node_system::protocol::{
    InputConsumption, LiteralPolicy, NodeInterfaceProtocol, NodeTypeId, OutputProduction,
    ParameterConstraint, ParameterKey, PortDirection, PortKey, Value,
};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn every_legacy_dataframe_node_has_one_stable_id() {
    assert_eq!(LEGACY_NODE_IDS.len(), 26);
    assert_eq!(NODES.len(), LEGACY_NODE_IDS.len());

    let legacy = LEGACY_NODE_IDS
        .iter()
        .map(|(name, _)| *name)
        .collect::<BTreeSet<_>>();
    let ids = LEGACY_NODE_IDS
        .iter()
        .map(|(_, id)| *id)
        .collect::<BTreeSet<_>>();

    assert_eq!(legacy.len(), LEGACY_NODE_IDS.len());
    assert_eq!(ids.len(), LEGACY_NODE_IDS.len());
    assert!(ids.iter().all(|id| id.starts_with("yssbi.dataframe.")));
    for spec in NODES {
        assert_eq!(
            LEGACY_NODE_IDS
                .iter()
                .find(|(legacy_name, _)| *legacy_name == spec.legacy_name)
                .map(|(_, id)| *id),
            Some(spec.id),
        );
    }
}

#[test]
fn dataframe_fragment_contains_every_migrated_protocol() {
    let fragment = build_provider_fragment();
    assert_eq!(fragment.nodes.len(), LEGACY_NODE_IDS.len());
}

#[test]
fn source_and_limit_freeze_and_lower_as_streaming_relational_nodes() {
    let registry = build_builtin_registry();
    let source_id = NodeTypeId::new("yssbi.dataframe.source.get").unwrap();
    let limit_id = NodeTypeId::new("yssbi.dataframe.limit").unwrap();
    let source = registry.get(&source_id).expect("source freezes");
    let limit = registry.get(&limit_id).expect("limit freezes");

    let source_output = source
        .protocol
        .interface
        .ports
        .iter()
        .find(|port| port.direction == PortDirection::Output)
        .expect("source output");
    assert_eq!(source_output.key.as_str(), "dataframe");
    assert_eq!(source_output.production, Some(OutputProduction::Streaming));

    let limit_input = limit
        .protocol
        .interface
        .ports
        .iter()
        .find(|port| port.direction == PortDirection::Input)
        .expect("limit input");
    let limit_output = limit
        .protocol
        .interface
        .ports
        .iter()
        .find(|port| port.direction == PortDirection::Output)
        .expect("limit output");
    assert_eq!(limit_input.key.as_str(), "source");
    assert_eq!(limit_input.consumption, Some(InputConsumption::Streaming));
    assert_eq!(limit_output.key.as_str(), "result");
    assert_eq!(limit_output.production, Some(OutputProduction::Streaming));

    let rows = limit
        .protocol
        .parameters
        .parameters
        .iter()
        .find(|parameter| parameter.key.as_str() == "rows")
        .expect("limit rows parameter");
    assert_eq!(
        rows.default_value.as_ref().map(|value| &value.value),
        Some(&Value::Integer(100)),
    );
    assert_eq!(
        rows.constraints,
        vec![ParameterConstraint::IntegerRange {
            min: Some(1),
            max: Some(1_000_000),
        }],
    );

    let source_node_id = NodeId::new();
    let source_parameters = BTreeMap::from([(
        ParameterKey::new("dataframe").unwrap(),
        serde_json::json!("databases/main"),
    )]);
    let source_output_address =
        PortAddress::declared(source_node_id, PortKey::new("dataframe").unwrap());
    let source_outputs = [(source_output_address, ValueRef::new(0))];
    let cancellation = CompileCancellationToken::new();
    let source_context = LoweringContext {
        cancellation: &cancellation,
        node_id: source_node_id,
        protocol: &source.protocol,
        parameters: &source_parameters,
        inputs: &[],
        outputs: &source_outputs,
    };
    let source_implementation = source
        .implementation
        .as_ref()
        .and_then(|implementation| implementation.as_any().downcast_ref::<NodeImplementation>())
        .expect("source compiler lowerer");
    let source_lowered = source_implementation
        .lowerer
        .lower(&source_context)
        .expect("source lowers");
    let LoweredKernel::Relational(source_fragment) = source_lowered.kernel else {
        panic!("source must lower relationally");
    };
    assert_eq!(source_fragment.backend.as_str(), "relational.default");
    assert!(source_fragment.inputs.is_empty());
    assert_eq!(
        source_fragment.fragment.root,
        RelationalOperatorIndex::new(0)
    );
    assert_eq!(
        source_fragment.fragment.operators.as_ref(),
        [RelationalOperator::Source {
            resource: ResourceId::new("databases/main").unwrap(),
            relation: "databases/main".into(),
        }],
    );

    let limit_node_id = NodeId::new();
    let limit_parameters =
        BTreeMap::from([(ParameterKey::new("rows").unwrap(), serde_json::json!(25))]);
    let limit_input_address = PortAddress::declared(limit_node_id, PortKey::new("source").unwrap());
    let limit_inputs = [(limit_input_address.clone(), ValueRef::new(0))];
    let limit_outputs = [(
        PortAddress::declared(limit_node_id, PortKey::new("result").unwrap()),
        ValueRef::new(1),
    )];
    let limit_context = LoweringContext {
        cancellation: &cancellation,
        node_id: limit_node_id,
        protocol: &limit.protocol,
        parameters: &limit_parameters,
        inputs: &limit_inputs,
        outputs: &limit_outputs,
    };
    let limit_implementation = limit
        .implementation
        .as_ref()
        .and_then(|implementation| implementation.as_any().downcast_ref::<NodeImplementation>())
        .expect("limit compiler lowerer");
    let limit_lowered = limit_implementation
        .lowerer
        .lower(&limit_context)
        .expect("limit lowers");
    let LoweredKernel::Relational(limit_fragment) = limit_lowered.kernel else {
        panic!("limit must lower relationally");
    };
    assert_eq!(limit_fragment.backend.as_str(), "relational.default");
    assert_eq!(
        limit_fragment.fragment.operators.as_ref(),
        [
            RelationalOperator::Input {
                name: "source".into(),
            },
            RelationalOperator::Limit {
                input: RelationalOperatorIndex::new(0),
                rows: 25,
            },
        ],
    );
    assert_eq!(
        limit_fragment.fragment.root,
        RelationalOperatorIndex::new(1)
    );
    assert_eq!(limit_fragment.inputs.len(), 1);
    assert_eq!(limit_fragment.inputs[0].port, limit_input_address);
    assert_eq!(limit_fragment.metadata.results.len(), 1);
    assert_eq!(
        limit_fragment.metadata.results[0].name.as_ref(),
        format!("node.{limit_node_id}.result")
    );
    assert_eq!(
        limit_fragment.inputs[0].operator,
        RelationalOperatorIndex::new(0),
    );
}

#[test]
fn other_dataframe_nodes_keep_native_lowerers() {
    let registry = build_builtin_registry();
    let cancellation = CompileCancellationToken::new();
    for spec in NODES.iter().filter(|spec| {
        !matches!(
            spec.id,
            "yssbi.dataframe.source.get" | "yssbi.dataframe.limit"
        )
    }) {
        let id = NodeTypeId::new(spec.id).unwrap();
        let node = registry.get(&id).expect("dataframe node freezes");
        let implementation = node
            .implementation
            .as_ref()
            .and_then(|implementation| implementation.as_any().downcast_ref::<NodeImplementation>())
            .expect("dataframe compiler lowerer");
        let context = LoweringContext {
            cancellation: &cancellation,
            node_id: NodeId::new(),
            protocol: &node.protocol,
            parameters: &BTreeMap::new(),
            inputs: &[],
            outputs: &[],
        };
        let lowered = implementation.lowerer.lower(&context).unwrap();
        assert!(
            matches!(lowered.kernel, LoweredKernel::Native(_)),
            "{}",
            spec.id,
        );
    }
}

#[test]
fn dataframe_protocols_have_unique_ports_and_valid_bindings() {
    for node in build_provider_fragment().nodes {
        let protocol = &node.protocol;
        let keys = protocol
            .interface
            .ports
            .iter()
            .map(|port| &port.key)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            keys.len(),
            protocol.interface.ports.len(),
            "{}",
            protocol.type_id
        );

        NodeInterfaceProtocol::new(
            protocol.interface.ports.to_vec(),
            protocol.interface.type_parameters.to_vec(),
            protocol.interface.type_constraints.to_vec(),
        )
        .unwrap_or_else(|error| panic!("{}: {error}", protocol.type_id));

        for port in &protocol.interface.ports {
            if let Some(default) = port
                .input_binding
                .as_ref()
                .and_then(|binding| binding.default_value.as_ref())
            {
                assert_eq!(
                    port.input_binding.as_ref().unwrap().literal_policy,
                    LiteralPolicy::Allowed,
                    "{}:{}",
                    protocol.type_id,
                    port.key
                );
                assert_eq!(
                    default.value_type, port.value_type,
                    "{}:{}",
                    protocol.type_id, port.key
                );
            }
        }
    }
}
