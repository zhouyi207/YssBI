use super::{LEGACY_NODE_IDS, NODES, build_provider_fragment};
use crate::node_system::catalog::builtin::build_builtin_node_system;
use crate::node_system::catalog::localization::Message;
use crate::node_system::compiler::{
    CompileCancellationToken, LoweredKernel, LoweringContext, NodeImplementation,
    ValidatedNodeConfig,
};
use crate::node_system::document::{NodeId, PortAddress};
use crate::node_system::plan::{
    KernelHandle, RelationalExpression, RelationalLiteral, RelationalOperator,
    RelationalOperatorIndex, RelationalProjection, RelationalRename, ResourceId, ValueRef,
};
use crate::node_system::protocol::{
    InputConsumption, LiteralPolicy, NodeInterfaceProtocol, NodeTypeId, OutputProduction,
    ParameterConstraint, ParameterEditorSpec, ParameterKey, PortDirection, PortKey, RenameExpr,
    SchemaExpr, TypeExpr, TypeId, Value,
};
use crate::node_system::registry::ImplementationKind;
use crate::node_system::runtime::build_builtin_kernel_registry;
use std::collections::{BTreeMap, BTreeSet};

fn validated_config(
    registry: &crate::node_system::registry::NodeRegistry,
    protocol: &crate::node_system::protocol::NodeProtocol,
    parameters: BTreeMap<ParameterKey, serde_json::Value>,
) -> ValidatedNodeConfig {
    ValidatedNodeConfig::from_analysis(protocol, parameters, |type_id, value| {
        registry.prepare_nominal_parameter(type_id, value)
    })
}

#[test]
fn every_legacy_dataframe_node_has_one_stable_id() {
    assert_eq!(LEGACY_NODE_IDS.len(), 26);
    assert_eq!(NODES.len(), LEGACY_NODE_IDS.len() + 3);

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
    for (legacy_name, id) in LEGACY_NODE_IDS {
        assert!(
            NODES
                .iter()
                .any(|spec| spec.legacy_name == Some(*legacy_name) && spec.id == *id)
        );
    }
}

#[test]
fn dataframe_fragment_contains_every_migrated_protocol() {
    let fragment = build_provider_fragment().expect("dataframe built-in fixture must assemble");
    assert_eq!(fragment.nodes.len(), LEGACY_NODE_IDS.len() + 3);
}

#[test]
fn rename_dataframe_freezes_exact_protocol_and_localization() {
    let fragment = build_provider_fragment().expect("dataframe built-in fixture must assemble");
    let rename_id = NodeTypeId::new("yssbi.dataframe.rename").unwrap();
    let rename = fragment
        .nodes
        .iter()
        .find(|node| node.protocol().type_id == rename_id)
        .expect("rename protocol");
    let protocol = &rename.protocol();

    assert_eq!(protocol.catalog.category_id.as_str(), "dataframe");
    assert_eq!(protocol.interface.ports.len(), 2);
    let source = &protocol.interface.ports[0];
    assert_eq!(source.key.as_str(), "source");
    assert_eq!(source.direction, PortDirection::Input);
    assert_eq!(
        source.value_type,
        TypeExpr::Concrete(TypeId::new("tabular.dataframe").unwrap())
    );
    assert_eq!(source.consumption, Some(InputConsumption::Streaming));
    let result = &protocol.interface.ports[1];
    assert_eq!(result.key.as_str(), "result");
    assert_eq!(result.direction, PortDirection::Output);
    assert_eq!(
        result.value_type,
        TypeExpr::Concrete(TypeId::new("tabular.dataframe").unwrap())
    );
    assert_eq!(result.production, Some(OutputProduction::Streaming));
    assert_eq!(
        result.schema,
        Some(SchemaExpr::Rename {
            input: Box::new(SchemaExpr::Input(PortKey::new("source").unwrap())),
            mapping: RenameExpr::FromParameters {
                from: ParameterKey::new("from").unwrap(),
                to: ParameterKey::new("to").unwrap(),
            },
        }),
    );

    assert_eq!(protocol.parameters.parameters.len(), 2);
    for (parameter, key) in protocol.parameters.parameters.iter().zip(["from", "to"]) {
        assert_eq!(parameter.key.as_str(), key);
        assert_eq!(
            parameter.value_type,
            TypeExpr::Concrete(TypeId::new("core.string").unwrap())
        );
        assert_eq!(
            parameter.editor,
            ParameterEditorSpec::Text { multiline: false }
        );
        assert_eq!(parameter.default_value, None);
        assert_eq!(parameter.constraints, vec![ParameterConstraint::Required]);
    }

    let messages = fragment
        .messages
        .iter()
        .map(|(locale, key, message)| ((*locale, *key), message))
        .collect::<BTreeMap<_, _>>();
    for (locale, expected) in [
        (
            "en-US",
            [
                ("nodes.yssbi.dataframe.rename.title", "Rename DataFrame"),
                (
                    "nodes.yssbi.dataframe.rename.description",
                    "Renames one DataFrame column.",
                ),
                (
                    "nodes.yssbi.dataframe.rename.documentation",
                    "Renames the column identified by 'from' to 'to'.",
                ),
                ("ports.source.label", "Source"),
                ("ports.result.label", "Result"),
                ("parameters.from.title", "Source column"),
                ("parameters.from.description", "Column name to rename."),
                ("parameters.to.title", "Destination column"),
                ("parameters.to.description", "New column name."),
            ],
        ),
        (
            "zh-CN",
            [
                ("nodes.yssbi.dataframe.rename.title", "重命名数据框"),
                (
                    "nodes.yssbi.dataframe.rename.description",
                    "重命名数据框中的一列。",
                ),
                (
                    "nodes.yssbi.dataframe.rename.documentation",
                    "将“源列”指定的列重命名为“目标列”。",
                ),
                ("ports.source.label", "源数据框"),
                ("ports.result.label", "结果"),
                ("parameters.from.title", "源列"),
                ("parameters.from.description", "要重命名的列名。"),
                ("parameters.to.title", "目标列"),
                ("parameters.to.description", "新的列名。"),
            ],
        ),
    ] {
        for (key, text) in expected {
            assert_eq!(messages.get(&(locale, key)), Some(&&Message::Text(text)));
        }
    }
}

#[test]
fn project_and_filter_rows_are_parameterized_catalog_nodes() {
    let fragment = build_provider_fragment().expect("dataframe built-in fixture must assemble");
    let builtin = build_builtin_node_system().unwrap();
    let registry = builtin.registry;
    let catalog = builtin.catalog;
    let localized = catalog.localize(&registry, "en-US");

    for (node_type, parameter_key, parameter_type, title, search_term) in [
        (
            "yssbi.dataframe.project",
            "columns",
            "yssbi.dataframe.project_columns",
            "Project DataFrame",
            "select columns",
        ),
        (
            "yssbi.dataframe.filter.rows",
            "predicate",
            "yssbi.dataframe.filter_predicate",
            "Filter Rows",
            "where rows",
        ),
    ] {
        let registered = fragment
            .nodes
            .iter()
            .find(|node| node.protocol().type_id.as_str() == node_type)
            .expect("new relational node protocol");
        assert!(registered.implementation().is_some());
        assert!(registered.structural_role().is_none());
        assert_eq!(registered.protocol().interface.ports.len(), 2);
        assert_eq!(
            registered.protocol().interface.ports[0].key.as_str(),
            "source"
        );
        assert_eq!(
            registered.protocol().interface.ports[0].consumption,
            Some(InputConsumption::Streaming),
        );
        assert_eq!(
            registered.protocol().interface.ports[1].key.as_str(),
            "result"
        );
        assert_eq!(
            registered.protocol().interface.ports[1].production,
            Some(OutputProduction::Streaming),
        );
        let parameter = &registered.protocol().parameters.parameters[0];
        assert_eq!(parameter.key.as_str(), parameter_key);
        assert_eq!(
            parameter.value_type,
            TypeExpr::Concrete(TypeId::new(parameter_type).unwrap()),
        );
        assert_eq!(parameter.default_value, None);
        assert_eq!(parameter.constraints, vec![ParameterConstraint::Required]);

        let item = localized
            .items
            .iter()
            .find(|item| item.node_type_id.as_ref() == node_type)
            .expect("parameterized node is catalog visible");
        assert_eq!(item.title.as_ref(), title);
        assert!(
            item.description
                .as_deref()
                .is_some_and(|value| !value.is_empty())
        );
        assert!(
            item.documentation
                .as_deref()
                .is_some_and(|value| !value.is_empty())
        );
        assert!(item.search_text.contains(search_term));
        assert_eq!(
            item.creation,
            crate::node_system::catalog::NodeCreationDescriptor::ParameterizedStatic {
                node_type_id: NodeTypeId::new(node_type).unwrap(),
                required_parameters: Box::new([ParameterKey::new(parameter_key).unwrap()]),
            },
        );
    }
}

#[test]
fn project_and_filter_rows_use_relational_lowerers_without_native_kernels() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let kernels = build_builtin_kernel_registry();

    for node_type in ["yssbi.dataframe.project", "yssbi.dataframe.filter.rows"] {
        let node_type = NodeTypeId::new(node_type).unwrap();
        let node = registry.get(&node_type).expect("relational node freezes");
        assert!(node.implementation().is_some(), "{node_type}");
        assert!(node.structural_role().is_none(), "{node_type}");
        assert!(
            kernels
                .get(&KernelHandle::new(node_type.as_str()).unwrap())
                .is_none(),
            "{node_type} must use a relational lowerer, not a native kernel",
        );
    }
}

#[test]
fn project_lowerer_preserves_exact_order() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let node = registry
        .get(&NodeTypeId::new("yssbi.dataframe.project").unwrap())
        .unwrap();
    let node_id = NodeId::new();
    let input = PortAddress::declared(node_id, PortKey::new("source").unwrap());
    let output = PortAddress::declared(node_id, PortKey::new("result").unwrap());
    let inputs = [(input.clone(), ValueRef::new(0))];
    let outputs = [(output.clone(), ValueRef::new(1))];
    let cancellation = CompileCancellationToken::new();
    let implementation = node
        .implementation()
        .as_ref()
        .and_then(|value| value.as_any().downcast_ref::<NodeImplementation>())
        .unwrap();

    let parameters = BTreeMap::from([(
        ParameterKey::new("columns").unwrap(),
        serde_json::json!(["b", "a"]),
    )]);
    let parameters = validated_config(&registry, &node.protocol(), parameters);
    let context = LoweringContext {
        cancellation: &cancellation,
        node_id,
        protocol: &node.protocol(),
        parameters: &parameters,
        inputs: &inputs,
        outputs: &outputs,
    };
    let lowered = implementation.lowerer.lower(&context).unwrap();
    let LoweredKernel::Relational(fragment) = lowered.kernel else {
        panic!("project must lower relationally");
    };
    assert_eq!(
        fragment.fragment.operators.as_ref(),
        [
            RelationalOperator::Input {
                name: "source".into(),
            },
            RelationalOperator::Project {
                input: RelationalOperatorIndex::new(0),
                columns: Box::new([
                    RelationalProjection {
                        name: "b".into(),
                        expression: RelationalExpression::Column("b".into()),
                    },
                    RelationalProjection {
                        name: "a".into(),
                        expression: RelationalExpression::Column("a".into()),
                    },
                ]),
            },
        ]
    );
    assert_eq!(fragment.fragment.root, RelationalOperatorIndex::new(1));
    assert_eq!(fragment.inputs[0].port, input);
    assert_eq!(fragment.metadata.results[0].output, output);
}

#[test]
fn filter_rows_lowerer_maps_every_operator_and_literal_exactly() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let node = registry
        .get(&NodeTypeId::new("yssbi.dataframe.filter.rows").unwrap())
        .unwrap();
    let node_id = NodeId::new();
    let input = PortAddress::declared(node_id, PortKey::new("source").unwrap());
    let output = PortAddress::declared(node_id, PortKey::new("result").unwrap());
    let inputs = [(input.clone(), ValueRef::new(0))];
    let outputs = [(output.clone(), ValueRef::new(1))];
    let cancellation = CompileCancellationToken::new();
    let implementation = node
        .implementation()
        .as_ref()
        .and_then(|value| value.as_any().downcast_ref::<NodeImplementation>())
        .unwrap();
    let column = || RelationalExpression::Column("value".into());
    let cases = [
        (
            serde_json::json!({"column":"value","operator":"equal","value":{"type":"boolean","value":true}}),
            RelationalExpression::Equal(
                Box::new(column()),
                Box::new(RelationalExpression::Literal(RelationalLiteral::Boolean(
                    true,
                ))),
            ),
        ),
        (
            serde_json::json!({"column":"value","operator":"notEqual","value":{"type":"integer","value":"42"}}),
            RelationalExpression::NotEqual(
                Box::new(column()),
                Box::new(RelationalExpression::Literal(RelationalLiteral::Integer(
                    42,
                ))),
            ),
        ),
        (
            serde_json::json!({"column":"value","operator":"lessThan","value":{"type":"decimal","value":"10.5"}}),
            RelationalExpression::LessThan(
                Box::new(column()),
                Box::new(RelationalExpression::Literal(RelationalLiteral::Decimal(
                    crate::node_system::protocol::CanonicalDecimal::new("10.5").unwrap(),
                ))),
            ),
        ),
        (
            serde_json::json!({"column":"value","operator":"lessThanOrEqual","value":{"type":"string","value":"paid"}}),
            RelationalExpression::LessThanOrEqual(
                Box::new(column()),
                Box::new(RelationalExpression::Literal(RelationalLiteral::String(
                    "paid".into(),
                ))),
            ),
        ),
        (
            serde_json::json!({"column":"value","operator":"greaterThan","value":{"type":"integer","value":"42"}}),
            RelationalExpression::GreaterThan(
                Box::new(column()),
                Box::new(RelationalExpression::Literal(RelationalLiteral::Integer(
                    42,
                ))),
            ),
        ),
        (
            serde_json::json!({"column":"value","operator":"greaterThanOrEqual","value":{"type":"integer","value":"42"}}),
            RelationalExpression::GreaterThanOrEqual(
                Box::new(column()),
                Box::new(RelationalExpression::Literal(RelationalLiteral::Integer(
                    42,
                ))),
            ),
        ),
        (
            serde_json::json!({"column":"value","operator":"isNull"}),
            RelationalExpression::IsNull(Box::new(column())),
        ),
        (
            serde_json::json!({"column":"value","operator":"isNotNull"}),
            RelationalExpression::Not(Box::new(RelationalExpression::IsNull(Box::new(column())))),
        ),
    ];

    for (wire, expected) in cases {
        let parameters = BTreeMap::from([(ParameterKey::new("predicate").unwrap(), wire)]);
        let parameters = validated_config(&registry, &node.protocol(), parameters);
        let context = LoweringContext {
            cancellation: &cancellation,
            node_id,
            protocol: &node.protocol(),
            parameters: &parameters,
            inputs: &inputs,
            outputs: &outputs,
        };
        let lowered = implementation.lowerer.lower(&context).unwrap();
        let LoweredKernel::Relational(fragment) = lowered.kernel else {
            panic!("filter rows must lower relationally");
        };
        assert_eq!(
            fragment.fragment.operators[1],
            RelationalOperator::Filter {
                input: RelationalOperatorIndex::new(0),
                predicate: expected,
            }
        );
        assert_eq!(fragment.fragment.root, RelationalOperatorIndex::new(1));
        assert_eq!(fragment.inputs[0].port, input);
        assert_eq!(fragment.metadata.results[0].output, output);
    }
}

#[test]
fn project_and_filter_rows_do_not_change_external_filter_or_decompose() {
    let fragment = build_provider_fragment().expect("dataframe built-in fixture must assemble");
    let filter = fragment
        .nodes
        .iter()
        .find(|node| node.protocol().type_id.as_str() == "yssbi.dataframe.filter")
        .expect("external mask filter remains registered");
    assert!(filter.protocol().parameters.parameters.is_empty());
    assert_eq!(
        filter
            .protocol()
            .interface
            .ports
            .iter()
            .map(|port| port.key.as_str())
            .collect::<Vec<_>>(),
        ["source", "condition", "result"],
    );
    let external_filter_schema = filter.protocol().interface.ports[2]
        .schema
        .as_ref()
        .expect("external filter output schema");
    assert_eq!(
        external_filter_schema,
        &SchemaExpr::Filter {
            input: Box::new(SchemaExpr::Input(PortKey::new("source").unwrap())),
            predicate: None,
        },
    );
    assert_eq!(
        serde_json::to_value(external_filter_schema).unwrap(),
        serde_json::json!({
            "Filter": {
                "input": { "Input": "source" }
            }
        }),
    );

    let decompose = fragment
        .nodes
        .iter()
        .find(|node| node.protocol().type_id.as_str() == "yssbi.dataframe.decompose")
        .expect("decompose remains registered");
    assert!(decompose.protocol().parameters.parameters.is_empty());
    assert_eq!(
        decompose
            .protocol()
            .interface
            .ports
            .iter()
            .map(|port| port.key.as_str())
            .collect::<Vec<_>>(),
        ["dataframe", "columns"],
    );
}

#[test]
fn rename_dataframe_is_excluded_from_static_catalog() {
    let builtin = build_builtin_node_system().unwrap();
    let registry = builtin.registry;
    let catalog = builtin.catalog;
    let localized = catalog.localize(&registry, "en-US");
    let node_type_ids = localized
        .items
        .iter()
        .map(|item| item.node_type_id.as_ref())
        .collect::<BTreeSet<_>>();

    assert!(!node_type_ids.contains("yssbi.dataframe.rename"));
}

#[test]
fn rename_dataframe_lowers_to_exact_input_and_rename_fragment() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let rename_id = NodeTypeId::new("yssbi.dataframe.rename").unwrap();
    let rename = registry.get(&rename_id).expect("rename freezes");
    let node_id = NodeId::new();
    let parameters = BTreeMap::from([
        (
            ParameterKey::new("from").unwrap(),
            serde_json::json!("old_name"),
        ),
        (
            ParameterKey::new("to").unwrap(),
            serde_json::json!("new_name"),
        ),
    ]);
    let input_address = PortAddress::declared(node_id, PortKey::new("source").unwrap());
    let inputs = [(input_address.clone(), ValueRef::new(0))];
    let output_address = PortAddress::declared(node_id, PortKey::new("result").unwrap());
    let outputs = [(output_address.clone(), ValueRef::new(1))];
    let cancellation = CompileCancellationToken::new();
    let parameters = validated_config(&registry, &rename.protocol(), parameters);
    let context = LoweringContext {
        cancellation: &cancellation,
        node_id,
        protocol: &rename.protocol(),
        parameters: &parameters,
        inputs: &inputs,
        outputs: &outputs,
    };
    let implementation = rename
        .implementation()
        .as_ref()
        .and_then(|implementation| implementation.as_any().downcast_ref::<NodeImplementation>())
        .expect("rename compiler lowerer");

    let lowered = implementation
        .lowerer
        .lower(&context)
        .expect("rename lowers");
    let LoweredKernel::Relational(fragment) = lowered.kernel else {
        panic!("rename must lower relationally");
    };
    assert_eq!(fragment.backend.as_str(), "relational.default");
    assert_eq!(
        fragment.fragment.operators.as_ref(),
        [
            RelationalOperator::Input {
                name: "source".into(),
            },
            RelationalOperator::Rename {
                input: RelationalOperatorIndex::new(0),
                columns: Box::new([RelationalRename {
                    from: "old_name".into(),
                    to: "new_name".into(),
                }]),
            },
        ],
    );
    assert_eq!(fragment.fragment.root, RelationalOperatorIndex::new(1));
    assert_eq!(fragment.inputs.len(), 1);
    assert_eq!(fragment.inputs[0].port, input_address);
    assert_eq!(fragment.inputs[0].operator, RelationalOperatorIndex::new(0));
    assert_eq!(fragment.metadata.results.len(), 1);
    assert_eq!(fragment.metadata.results[0].output, output_address);
    assert_eq!(
        fragment.metadata.results[0].name.as_ref(),
        format!("node.{node_id}.result")
    );
}

#[test]
fn source_and_limit_freeze_and_lower_as_streaming_relational_nodes() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let source_id = NodeTypeId::new("yssbi.dataframe.source.get").unwrap();
    let limit_id = NodeTypeId::new("yssbi.dataframe.limit").unwrap();
    let source = registry.get(&source_id).expect("source freezes");
    let limit = registry.get(&limit_id).expect("limit freezes");

    let source_output = source
        .protocol()
        .interface
        .ports
        .iter()
        .find(|port| port.direction == PortDirection::Output)
        .expect("source output");
    assert_eq!(source_output.key.as_str(), "dataframe");
    assert_eq!(source_output.production, Some(OutputProduction::Streaming));

    let limit_input = limit
        .protocol()
        .interface
        .ports
        .iter()
        .find(|port| port.direction == PortDirection::Input)
        .expect("limit input");
    let limit_output = limit
        .protocol()
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
        .protocol()
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
    let source_parameters = validated_config(&registry, &source.protocol(), source_parameters);
    let source_context = LoweringContext {
        cancellation: &cancellation,
        node_id: source_node_id,
        protocol: &source.protocol(),
        parameters: &source_parameters,
        inputs: &[],
        outputs: &source_outputs,
    };
    let source_implementation = source
        .implementation()
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
    let limit_parameters = validated_config(&registry, &limit.protocol(), limit_parameters);
    let limit_context = LoweringContext {
        cancellation: &cancellation,
        node_id: limit_node_id,
        protocol: &limit.protocol(),
        parameters: &limit_parameters,
        inputs: &limit_inputs,
        outputs: &limit_outputs,
    };
    let limit_implementation = limit
        .implementation()
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
fn dataframe_native_lowerings_have_production_implementations() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let kernels = build_builtin_kernel_registry();
    let cancellation = CompileCancellationToken::new();

    for spec in NODES {
        let id = NodeTypeId::new(spec.id).unwrap();
        let node = registry.get(&id).expect("dataframe node freezes");
        let Some(implementation) = node.implementation() else {
            assert!(node.structural_role().is_none(), "{}", spec.id);
            continue;
        };
        assert_eq!(
            implementation.capability(),
            ImplementationKind::CompilerLowering
        );
        let implementation = implementation
            .as_any()
            .downcast_ref::<NodeImplementation>()
            .expect("dataframe compiler lowerer");
        let parameters = validated_config(&registry, &node.protocol(), BTreeMap::new());
        let context = LoweringContext {
            cancellation: &cancellation,
            node_id: NodeId::new(),
            protocol: &node.protocol(),
            parameters: &parameters,
            inputs: &[],
            outputs: &[],
        };
        let native_implementation = node
            .implementation()
            .as_ref()
            .expect("checked implementation")
            .implementation_identity()
            .ends_with("::KernelLowerer");
        let lowered = match implementation.lowerer.lower(&context) {
            Ok(lowered) => lowered,
            Err(error) if !native_implementation => {
                let _ = error;
                continue;
            }
            Err(error) => panic!("native node '{}' failed to lower: {error}", spec.id),
        };
        assert!(
            !native_implementation || matches!(lowered.kernel, LoweredKernel::Native(_)),
            "native implementation for '{}' emitted a non-native fragment",
            spec.id,
        );
        if let LoweredKernel::Native(handle) = lowered.kernel {
            assert!(
                kernels.get(&handle).is_some(),
                "{} emits missing native kernel '{}'",
                spec.id,
                handle.as_str(),
            );
        }
    }
}

#[test]
fn dataframe_protocols_have_unique_ports_and_valid_bindings() {
    for node in build_provider_fragment()
        .expect("dataframe built-in fixture must assemble")
        .nodes
    {
        let protocol = &node.protocol();
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
