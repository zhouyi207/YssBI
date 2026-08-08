use super::materialization::ProjectedMemberRef;
use super::mutation::{create_node_operations, validate_parameters};
use super::*;
use crate::node_system::analysis::ResourceVersionSet;
use crate::node_system::catalog::{
    build_builtin_node_system, builtin_bundle_parts_for_test, validate_builtin_bundle_for_test,
};
use crate::node_system::compiler::{
    GraphCompiler, LoweredNode, LoweringContext, LoweringError, NodeImplementation, NodeLowerer,
    ResourceSnapshot,
};
use crate::node_system::protocol::{
    CachePolicy, ConnectionsPerPort, Determinism, EffectSemantics, EvaluationPolicy, I18nKey,
    InputBindingSpec, LiteralPolicy, NodeCategoryId, NodeScope, NodeTypeId, ParameterKey,
    PortDirection, PortEditorSpec, PortInstances, PortKey, PortKind, PortSpec, ProviderId, Purity,
    TypeExpr,
};
use crate::node_system::registry::{
    CategoryRegistration, I18nManifest, NodeRegistry, NodeRegistryBuilder, ProviderRegistration,
    RegisteredNode,
};
use crate::node_system::testing::TestProtocolBuilder;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use uuid::Uuid;

mod editor_mutation_validation;

fn node_id(value: u128) -> NodeId {
    NodeId::from_uuid(Uuid::from_u128(value))
}

fn instance_id(value: u128) -> PortInstanceId {
    PortInstanceId::from_uuid(Uuid::from_u128(value))
}

fn connection_id(value: u128) -> ConnectionId {
    ConnectionId::from_uuid(Uuid::from_u128(value))
}

fn operation_id(value: u128) -> OperationId {
    OperationId::from_uuid(Uuid::from_u128(value))
}

fn graph_path(value: &str) -> GraphResourcePath {
    GraphResourcePath(value.into())
}

fn node(id: NodeId) -> DocumentNode {
    DocumentNode {
        id,
        node_type: NodeTypeId::new("yssbi.test.node").unwrap(),
        position: NodePosition { x: 1.0, y: 2.0 },
        parameters: ParameterValues::new(),
        user_label: None,
    }
}

fn declared(id: NodeId, key: &str) -> PortAddress {
    PortAddress::declared(id, PortKey::new(key).unwrap())
}

fn assert_graph_content_eq(left: &GraphDocument, right: &GraphDocument) {
    assert_eq!(left.nodes, right.nodes);
    assert_eq!(left.port_bindings, right.port_bindings);
    assert_eq!(left.connections, right.connections);
    assert_eq!(left.input_states, right.input_states);
}

fn binding() -> DynamicPortBinding {
    DynamicPortBinding::Resolved {
        origin: DynamicMemberLocator::SchemaField {
            source: SchemaSourceIdentity("source".into()),
            field: SchemaFieldIdentity("field".into()),
        },
        order: OrderKey("a".into()),
    }
}

const EDITOR_MUTATION_EXECUTION: crate::node_system::protocol::ExecutionSemantics =
    crate::node_system::protocol::ExecutionSemantics {
        determinism: Determinism::Deterministic,
        purity: Purity::Pure,
        evaluation: EvaluationPolicy::DemandDriven,
        cache: CachePolicy::PerRun,
        effects: EffectSemantics::None,
        idempotent: false,
        retry: None,
    };

struct EditorMutationTestLowerer;

impl NodeLowerer for EditorMutationTestLowerer {
    fn lower(&self, _: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        unreachable!("mutation planning never lowers nodes")
    }
}

fn editor_mutation_registry() -> NodeRegistry {
    editor_mutation_registry_with(NodeScope::Any, 1)
}

fn editor_mutation_registry_with(scope: NodeScope, minimum_inputs: u16) -> NodeRegistry {
    let protocol = TestProtocolBuilder::new("yssbi.test.editor_mutation", "test")
        .style("test")
        .ports(vec![
            PortSpec {
                key: PortKey::new("output").unwrap(),
                label_key: I18nKey::new("nodes.test.editor_mutation.output").unwrap(),
                direction: PortDirection::Output,
                kind: PortKind::Data,
                value_type: TypeExpr::Unknown,
                instances: PortInstances::Declared,
                connections: ConnectionsPerPort::Multiple {
                    max: None,
                    ordered: false,
                },
                input_binding: None,
                consumption: None,
                production: None,
                editor: PortEditorSpec::Default,
                schema: None,
            },
            PortSpec {
                key: PortKey::new("inputs").unwrap(),
                label_key: I18nKey::new("nodes.test.editor_mutation.inputs").unwrap(),
                direction: PortDirection::Input,
                kind: PortKind::Data,
                value_type: TypeExpr::Unknown,
                instances: PortInstances::UserCreated {
                    min: minimum_inputs,
                    max: Some(minimum_inputs.max(2)),
                },
                connections: ConnectionsPerPort::Single,
                input_binding: Some(InputBindingSpec {
                    literal_policy: LiteralPolicy::Allowed,
                    default_value: None,
                }),
                consumption: None,
                production: None,
                editor: PortEditorSpec::Default,
                schema: None,
            },
        ])
        .execution(EDITOR_MUTATION_EXECUTION)
        .scope(scope)
        .build();
    let mut provider = ProviderRegistration::new(ProviderId::new("yssbi").unwrap());
    provider.categories = vec![CategoryRegistration {
        id: NodeCategoryId::new("test").unwrap(),
        title_key: I18nKey::new("categories.test.title").unwrap(),
        parent: None,
        order: 0,
    }]
    .into_boxed_slice();
    provider.i18n = I18nManifest {
        keys: BTreeSet::from([
            I18nKey::new("categories.test.title").unwrap(),
            I18nKey::new("nodes.test.editor_mutation.title").unwrap(),
            I18nKey::new("nodes.test.editor_mutation.output").unwrap(),
            I18nKey::new("nodes.test.editor_mutation.inputs").unwrap(),
        ]),
    };
    provider.nodes = vec![RegisteredNode::leaf(
        Arc::new(protocol),
        Arc::new(NodeImplementation::new(EditorMutationTestLowerer)),
    )]
    .into_boxed_slice();
    let mut builder = NodeRegistryBuilder::new();
    builder.register_provider(provider).unwrap();
    builder.freeze().unwrap()
}

fn editor_mutation_node(id: NodeId) -> DocumentNode {
    DocumentNode {
        id,
        node_type: NodeTypeId::new("yssbi.test.editor_mutation").unwrap(),
        position: NodePosition { x: 1.0, y: 2.0 },
        parameters: ParameterValues::new(),
        user_label: None,
    }
}

fn builtin_control_node(id: NodeId, node_type: &str) -> DocumentNode {
    DocumentNode {
        id,
        node_type: NodeTypeId::new(node_type).unwrap(),
        position: NodePosition { x: 1.0, y: 2.0 },
        parameters: ParameterValues::new(),
        user_label: None,
    }
}

fn builtin_registry_with_branch_group_max(max: u16) -> NodeRegistry {
    let (mut provider, catalog, alias_keys) = builtin_bundle_parts_for_test().unwrap();
    let branch_index = provider
        .nodes
        .iter()
        .position(|node| node.protocol().type_id.as_str() == "yssbi.control.branch")
        .unwrap();
    let mut protocol = provider.nodes[branch_index].protocol().clone();
    protocol.interface.member_groups[0].max = Some(max);
    provider.nodes[branch_index] = RegisteredNode::structural(
        Arc::new(protocol),
        crate::node_system::registry::StructuralNodeRole::Branch,
    );
    Arc::unwrap_or_clone(
        validate_builtin_bundle_for_test(provider, catalog, alias_keys)
            .unwrap()
            .registry,
    )
}

fn bind_user_port(
    document: &mut GraphDocument,
    node_id: NodeId,
    template: &str,
    instance_id: PortInstanceId,
) {
    document
        .bind_port(
            PortAddress::instance(node_id, PortKey::new(template).unwrap(), instance_id),
            DynamicPortBinding::UserCreated {
                order: OrderKey(instance_id.to_string().into()),
            },
        )
        .unwrap();
}

fn grouped_binding_addresses(patch: &GraphDocumentPatch) -> Vec<PortAddress> {
    patch
        .operations
        .iter()
        .filter_map(|operation| match operation {
            GraphDocumentOperation::InsertPortBinding {
                address,
                binding: DynamicPortBinding::UserCreated { .. },
            } => Some(address.clone()),
            _ => None,
        })
        .collect()
}

fn instance_identity(address: &PortAddress) -> PortInstanceId {
    match &address.port {
        PortRef::Instance { instance_id, .. } => *instance_id,
        PortRef::Declared { .. } => panic!("expected instance address"),
    }
}

fn instance_template(address: &PortAddress) -> &str {
    match &address.port {
        PortRef::Instance { template, .. } => template.as_str(),
        PortRef::Declared { .. } => panic!("expected instance address"),
    }
}

#[test]
fn canonical_editor_mutation_address_wire_declared() {
    let mutation = EditorGraphMutationDto::SetLiteral {
        address: declared(node_id(901), "value").into(),
        literal: Some(json!(42)),
    };
    let expected = json!({
        "type": "setLiteral",
        "payload": {
            "address": {
                "kind": "declared",
                "nodeId": "00000000-0000-0000-0000-000000000385",
                "portKey": "value"
            },
            "literal": 42
        }
    });

    assert_eq!(serde_json::to_value(&mutation).unwrap(), expected);
    assert_eq!(
        serde_json::from_value::<EditorGraphMutationDto>(expected).unwrap(),
        mutation
    );
}

#[test]
fn canonical_editor_mutation_address_wire_instance() {
    let mutation = EditorGraphMutationDto::RemovePortInstance {
        address: PortAddress::instance(
            node_id(902),
            PortKey::new("inputs").unwrap(),
            instance_id(904),
        )
        .into(),
    };
    let expected = json!({
        "type": "removePortInstance",
        "payload": {
            "address": {
                "kind": "instance",
                "nodeId": "00000000-0000-0000-0000-000000000386",
                "templateKey": "inputs",
                "instanceId": "00000000-0000-0000-0000-000000000388"
            }
        }
    });

    assert_eq!(serde_json::to_value(&mutation).unwrap(), expected);
    assert_eq!(
        serde_json::from_value::<EditorGraphMutationDto>(expected).unwrap(),
        mutation
    );
}

#[test]
fn editor_mutation_wire_is_stable_and_camel_case() {
    let first = node_id(901);
    let second = node_id(902);
    let connection = connection_id(903);
    let instance = instance_id(904);
    let output = PortAddressDto::from(declared(first, "output"));
    let input = PortAddressDto::from(PortAddress::instance(
        second,
        PortKey::new("inputs").unwrap(),
        instance,
    ));
    let cases = [
        (
            EditorGraphMutationDto::CreateNode {
                descriptor: crate::node_system::catalog::NodeCreationDescriptor::Static {
                    node_type_id: NodeTypeId::new("yssbi.test.editor_mutation").unwrap(),
                },
                position: NodePosition { x: 1.0, y: 2.0 },
                user_label: Some("Created".to_owned()),
            },
            json!({
                "type": "createNode",
                "payload": {
                    "descriptor": {
                        "kind": "static",
                        "nodeTypeId": "yssbi.test.editor_mutation"
                    },
                    "position": { "x": 1.0, "y": 2.0 },
                    "userLabel": "Created"
                }
            }),
        ),
        (
            EditorGraphMutationDto::DeleteNode { node_id: first },
            json!({ "type": "deleteNode", "payload": { "nodeId": first } }),
        ),
        (
            EditorGraphMutationDto::MoveNodes {
                positions: vec![NodePositionMutationDto {
                    node_id: first,
                    position: NodePosition { x: 3.0, y: 4.0 },
                }],
            },
            json!({
                "type": "moveNodes",
                "payload": {
                    "positions": [{ "nodeId": first, "position": { "x": 3.0, "y": 4.0 } }]
                }
            }),
        ),
        (
            EditorGraphMutationDto::Connect {
                output: output.clone(),
                input: input.clone(),
                order: Some(OrderKey("a".into())),
            },
            json!({
                "type": "connect",
                "payload": { "output": output, "input": input.clone(), "order": "a" }
            }),
        ),
        (
            EditorGraphMutationDto::Disconnect {
                connection_id: connection,
            },
            json!({ "type": "disconnect", "payload": { "connectionId": connection } }),
        ),
        (
            EditorGraphMutationDto::SetLiteral {
                address: input.clone(),
                literal: Some(json!(42)),
            },
            json!({
                "type": "setLiteral",
                "payload": { "address": input.clone(), "literal": 42 }
            }),
        ),
        (
            EditorGraphMutationDto::AddPortInstance {
                node_id: second,
                template: PortKey::new("inputs").unwrap(),
                order: None,
            },
            json!({
                "type": "addPortInstance",
                "payload": { "nodeId": second, "template": "inputs", "order": null }
            }),
        ),
        (
            EditorGraphMutationDto::RemovePortInstance {
                address: input.clone(),
            },
            json!({
                "type": "removePortInstance",
                "payload": { "address": input }
            }),
        ),
    ];

    for (mutation, expected) in cases {
        let serialized = serde_json::to_value(&mutation).unwrap();
        assert_eq!(serialized, expected);
        assert_eq!(
            serde_json::from_value::<EditorGraphMutationDto>(serialized).unwrap(),
            mutation
        );
    }
}

#[test]
fn parameterized_static_creation_is_editable_with_empty_parameters() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let patch = EditorGraphMutationDto::CreateNode {
        descriptor: crate::node_system::catalog::NodeCreationDescriptor::ParameterizedStatic {
            node_type_id: NodeTypeId::new("yssbi.dataframe.project").unwrap(),
            required_parameters: Box::new([ParameterKey::new("columns").unwrap()]),
        },
        position: NodePosition { x: 1.0, y: 2.0 },
        user_label: None,
    }
    .into_patch(
        &graph_path("events/parameterized"),
        &GraphDocument::default(),
        &registry,
    )
    .unwrap();

    let GraphDocumentOperation::InsertNode { node } = &patch.operations[0] else {
        panic!("parameterized creation must insert a node");
    };
    assert_eq!(node.node_type.as_str(), "yssbi.dataframe.project");
    assert!(node.parameters.is_empty());
}

#[test]
fn parameterized_static_missing_parameter_remains_compile_blocking() {
    struct EmptyResources;
    impl ResourceSnapshot for EmptyResources {
        fn versions(&self) -> ResourceVersionSet {
            ResourceVersionSet::new()
        }
    }

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let node_id = node_id(989);
    let mut document = GraphDocument::default();
    document
        .create_node(DocumentNode {
            id: node_id,
            node_type: NodeTypeId::new("yssbi.dataframe.project").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        })
        .unwrap();

    let compiled = GraphCompiler::new(&registry, &EmptyResources).compile(&document);

    assert!(compiled.semantic.is_none());
    assert!(compiled.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "compiler.parameter.required"
            && matches!(
                &diagnostic.primary,
                crate::node_system::analysis::DiagnosticLocation::Parameter {
                    node_id: diagnostic_node,
                    key,
                } if *diagnostic_node == node_id && key.as_str() == "columns"
            )
    }));
}

#[test]
fn forged_parameterized_static_descriptors_have_zero_effects() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let document = GraphDocument::default();
    let project = NodeTypeId::new("yssbi.dataframe.project").unwrap();
    let filter = NodeTypeId::new("yssbi.dataframe.filter.rows").unwrap();
    let columns = ParameterKey::new("columns").unwrap();
    let predicate = ParameterKey::new("predicate").unwrap();
    let descriptors = [
        crate::node_system::catalog::NodeCreationDescriptor::Static {
            node_type_id: project.clone(),
        },
        crate::node_system::catalog::NodeCreationDescriptor::ParameterizedStatic {
            node_type_id: project.clone(),
            required_parameters: Box::new([]),
        },
        crate::node_system::catalog::NodeCreationDescriptor::ParameterizedStatic {
            node_type_id: project.clone(),
            required_parameters: Box::new([columns.clone(), predicate.clone()]),
        },
        crate::node_system::catalog::NodeCreationDescriptor::ParameterizedStatic {
            node_type_id: project.clone(),
            required_parameters: Box::new([columns.clone(), columns.clone()]),
        },
        crate::node_system::catalog::NodeCreationDescriptor::ParameterizedStatic {
            node_type_id: project,
            required_parameters: Box::new([predicate]),
        },
        crate::node_system::catalog::NodeCreationDescriptor::ParameterizedStatic {
            node_type_id: filter,
            required_parameters: Box::new([columns]),
        },
        crate::node_system::catalog::NodeCreationDescriptor::ParameterizedStatic {
            node_type_id: NodeTypeId::new("yssbi.numeric.add.int64").unwrap(),
            required_parameters: Box::new([]),
        },
    ];

    for descriptor in descriptors {
        let result = EditorGraphMutationDto::CreateNode {
            descriptor,
            position: NodePosition { x: 1.0, y: 2.0 },
            user_label: None,
        }
        .into_patch(&graph_path("events/forged"), &document, &registry);
        assert!(result.is_err());
        assert!(document.nodes.is_empty());
    }
}

#[test]
fn set_parameters_atomically_replaces_and_validates_the_complete_map() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let node_id = node_id(990);
    let mut document = GraphDocument::default();
    document
        .create_node(DocumentNode {
            id: node_id,
            node_type: NodeTypeId::new("yssbi.dataframe.project").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        })
        .unwrap();
    let parameters = ParameterValues::from([(
        ParameterKey::new("columns").unwrap(),
        json!(["status", "amount"]),
    )]);
    let mutation = EditorGraphMutationDto::SetParameters {
        node_id,
        parameters: parameters.clone(),
    };
    assert_eq!(
        serde_json::to_value(&mutation).unwrap(),
        json!({
            "type": "setParameters",
            "payload": {
                "nodeId": node_id,
                "parameters": { "columns": ["status", "amount"] }
            }
        }),
    );

    let patch = mutation
        .into_patch(&graph_path("events/parameters"), &document, &registry)
        .unwrap();
    assert_eq!(patch.operations.len(), 1);
    let GraphDocumentOperation::UpdateNode { before, after } = &patch.operations[0] else {
        panic!("parameter update must be one node replacement");
    };
    assert!(before.parameters.is_empty());
    assert_eq!(after.parameters, parameters);
}

#[test]
fn invalid_atomic_parameter_mutations_have_zero_effects() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let node_id = node_id(991);
    let mut document = GraphDocument::default();
    document
        .create_node(DocumentNode {
            id: node_id,
            node_type: NodeTypeId::new("yssbi.dataframe.filter.rows").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        })
        .unwrap();
    for parameters in [
        ParameterValues::new(),
        ParameterValues::from([(
            ParameterKey::new("predicate").unwrap(),
            json!({
                "column": "count",
                "operator": "greaterThan",
                "value": { "type": "integer", "value": 9007199254740993_i64 }
            }),
        )]),
        ParameterValues::from([(ParameterKey::new("columns").unwrap(), json!(["forged"]))]),
    ] {
        let result = EditorGraphMutationDto::SetParameters {
            node_id,
            parameters,
        }
        .into_patch(&graph_path("events/parameters"), &document, &registry);
        assert!(result.is_err());
        assert!(document.nodes[&node_id].parameters.is_empty());
    }
}

#[test]
fn create_node_rejects_protocol_scope_mismatch() {
    let registry = editor_mutation_registry_with(NodeScope::Event, 0);
    let mutation = EditorGraphMutationDto::CreateNode {
        descriptor: crate::node_system::catalog::NodeCreationDescriptor::Static {
            node_type_id: NodeTypeId::new("yssbi.test.editor_mutation").unwrap(),
        },
        position: NodePosition { x: 1.0, y: 2.0 },
        user_label: None,
    };

    let error = mutation
        .into_patch(
            &graph_path("functions/scope-mismatch"),
            &GraphDocument::default(),
            &registry,
        )
        .unwrap_err();

    assert!(error.to_string().contains("scope"));
}

#[test]
fn create_node_materializes_required_user_created_ports() {
    let registry = editor_mutation_registry_with(NodeScope::Any, 2);
    let patch = EditorGraphMutationDto::CreateNode {
        descriptor: crate::node_system::catalog::NodeCreationDescriptor::Static {
            node_type_id: NodeTypeId::new("yssbi.test.editor_mutation").unwrap(),
        },
        position: NodePosition { x: 1.0, y: 2.0 },
        user_label: None,
    }
    .into_patch(
        &graph_path("events/initial-ports"),
        &GraphDocument::default(),
        &registry,
    )
    .unwrap();

    let node_id = match &patch.operations[0] {
        GraphDocumentOperation::InsertNode { node } => node.id,
        operation => panic!("expected node insertion first, got {operation:?}"),
    };
    let bindings = patch
        .operations
        .iter()
        .skip(1)
        .map(|operation| match operation {
            GraphDocumentOperation::InsertPortBinding {
                address,
                binding: DynamicPortBinding::UserCreated { .. },
            } => address,
            operation => panic!("unexpected create operation: {operation:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(bindings.len(), 2);
    assert!(bindings.iter().all(|address| address.node_id == node_id));
    assert_ne!(bindings[0], bindings[1]);
}

#[test]
fn builtin_loop_create_materializes_one_complete_carried_member() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut parameters = ParameterValues::new();
    parameters.insert(ParameterKey::new("max_iterations").unwrap(), json!(100));
    let node_type_id = NodeTypeId::new("yssbi.control.loop").unwrap();
    let protocol = registry.protocol(&node_type_id).unwrap();
    validate_parameters(protocol, &parameters).unwrap();
    let patch = GraphDocumentPatch::new(create_node_operations(
        protocol,
        node_type_id,
        NodePosition { x: 1.0, y: 2.0 },
        parameters,
        None,
    ));

    let addresses = grouped_binding_addresses(&patch);
    assert_eq!(addresses.len(), 4);
    assert_eq!(
        addresses
            .iter()
            .map(|address| instance_template(address))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["body_input", "initial_source", "next_source", "result"])
    );
    assert_eq!(
        addresses
            .iter()
            .map(instance_identity)
            .collect::<BTreeSet<_>>()
            .len(),
        1,
        "one carried member must share one identity across all templates"
    );
}

#[test]
fn builtin_branch_adds_complete_members_with_stable_shared_identities() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let path = graph_path("events/grouped-branch");
    let owner = node_id(905);
    let templates = ["then_source", "else_source", "result"];

    for requested in templates {
        let mut document = GraphDocument::default();
        document
            .create_node(builtin_control_node(owner, "yssbi.control.branch"))
            .unwrap();
        let patch = EditorGraphMutationDto::AddPortInstance {
            node_id: owner,
            template: PortKey::new(requested).unwrap(),
            order: Some(OrderKey("member".into())),
        }
        .into_patch(&path, &document, &registry)
        .unwrap();
        let addresses = grouped_binding_addresses(&patch);
        assert_eq!(addresses.len(), 3);
        assert_eq!(
            addresses
                .iter()
                .map(|address| instance_template(address))
                .collect::<BTreeSet<_>>(),
            templates.into_iter().collect()
        );
        assert_eq!(
            addresses
                .iter()
                .map(instance_identity)
                .collect::<BTreeSet<_>>()
                .len(),
            1
        );
    }

    let mut document = GraphDocument::default();
    document
        .create_node(builtin_control_node(owner, "yssbi.control.branch"))
        .unwrap();
    let first = EditorGraphMutationDto::AddPortInstance {
        node_id: owner,
        template: PortKey::new("result").unwrap(),
        order: Some(OrderKey("z".into())),
    }
    .into_patch(&path, &document, &registry)
    .unwrap();
    let first_id = instance_identity(&grouped_binding_addresses(&first)[0]);
    let mut reversed = first.operations.to_vec();
    reversed.reverse();
    document
        .apply_patch(&GraphDocumentPatch::new(reversed))
        .unwrap();

    let second = EditorGraphMutationDto::AddPortInstance {
        node_id: owner,
        template: PortKey::new("then_source").unwrap(),
        order: Some(OrderKey("a".into())),
    }
    .into_patch(&path, &document, &registry)
    .unwrap();
    let second_id = instance_identity(&grouped_binding_addresses(&second)[0]);
    assert_ne!(first_id, second_id);
    document.apply_patch(&second).unwrap();

    let mut by_identity = BTreeMap::<PortInstanceId, BTreeSet<&str>>::new();
    for address in document.port_bindings.keys() {
        by_identity
            .entry(instance_identity(address))
            .or_default()
            .insert(instance_template(address));
    }
    assert_eq!(by_identity.len(), 2);
    assert!(
        by_identity
            .values()
            .all(|members| { members == &templates.into_iter().collect::<BTreeSet<_>>() })
    );
}

#[test]
fn removing_any_group_member_atomically_removes_the_complete_member() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let path = graph_path("events/grouped-remove");
    let owner = node_id(906);
    let source = node_id(907);
    let sink = node_id(908);
    let mut document = GraphDocument::default();
    document
        .create_node(builtin_control_node(owner, "yssbi.control.branch"))
        .unwrap();
    document.create_node(node(source)).unwrap();
    document.create_node(node(sink)).unwrap();

    for order in ["first", "second"] {
        let patch = EditorGraphMutationDto::AddPortInstance {
            node_id: owner,
            template: PortKey::new("else_source").unwrap(),
            order: Some(OrderKey(order.into())),
        }
        .into_patch(&path, &document, &registry)
        .unwrap();
        document.apply_patch(&patch).unwrap();
    }
    let removed_id = document
        .port_bindings
        .keys()
        .map(instance_identity)
        .min()
        .unwrap();
    let grouped =
        |template| PortAddress::instance(owner, PortKey::new(template).unwrap(), removed_id);
    let then_source = grouped("then_source");
    let else_source = grouped("else_source");
    let result = grouped("result");
    document
        .connect(declared(source, "output"), then_source.clone(), None)
        .unwrap();
    document
        .connect(declared(source, "output_2"), else_source.clone(), None)
        .unwrap();
    document
        .connect(result.clone(), declared(sink, "input"), None)
        .unwrap();
    document
        .set_literal(then_source.clone(), Some(json!(1)))
        .unwrap();
    document
        .set_literal(else_source.clone(), Some(json!(2)))
        .unwrap();
    let before = document.clone();

    let patch = EditorGraphMutationDto::RemovePortInstance {
        address: else_source.into(),
    }
    .into_patch(&path, &document, &registry)
    .unwrap();
    document.apply_patch(&patch).unwrap();

    assert!(document.port_bindings.keys().all(|address| {
        !matches!(&address.port, PortRef::Instance { instance_id, .. } if *instance_id == removed_id)
    }));
    assert!(document.input_states.keys().all(|address| {
        !matches!(&address.port, PortRef::Instance { instance_id, .. } if *instance_id == removed_id)
    }));
    assert!(document.connections.values().all(|connection| {
        instance_identity_if_present(&connection.output) != Some(removed_id)
            && instance_identity_if_present(&connection.input) != Some(removed_id)
    }));
    assert_eq!(document.port_bindings.len(), 3);

    document.apply_patch(&patch.inverse()).unwrap();
    assert_graph_content_eq(&document, &before);
}

fn instance_identity_if_present(address: &PortAddress) -> Option<PortInstanceId> {
    match &address.port {
        PortRef::Instance { instance_id, .. } => Some(*instance_id),
        PortRef::Declared { .. } => None,
    }
}

#[test]
fn loop_partial_member_does_not_inflate_complete_count_or_block_repair() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let path = graph_path("events/partial-loop");
    let owner = node_id(909);
    let complete_id = instance_id(910);
    let partial_id = instance_id(911);
    let mut document = GraphDocument::default();
    document
        .create_node(builtin_control_node(owner, "yssbi.control.loop"))
        .unwrap();
    for template in ["initial_source", "body_input", "next_source", "result"] {
        bind_user_port(&mut document, owner, template, complete_id);
    }
    bind_user_port(&mut document, owner, "initial_source", partial_id);

    assert!(
        EditorGraphMutationDto::RemovePortInstance {
            address: PortAddress::instance(owner, PortKey::new("result").unwrap(), complete_id,)
                .into(),
        }
        .into_patch(&path, &document, &registry)
        .is_err(),
        "the only complete member must satisfy Loop min=1"
    );

    let remove_partial = EditorGraphMutationDto::RemovePortInstance {
        address: PortAddress::instance(owner, PortKey::new("initial_source").unwrap(), partial_id)
            .into(),
    }
    .into_patch(&path, &document, &registry)
    .unwrap();
    document.apply_patch(&remove_partial).unwrap();
    assert!(
        document
            .port_bindings
            .keys()
            .all(|address| { instance_identity_if_present(address) != Some(partial_id) })
    );
    assert_eq!(
        document
            .port_bindings
            .keys()
            .filter(|address| instance_identity_if_present(address) == Some(complete_id))
            .count(),
        4
    );
}

#[test]
fn loop_with_only_a_partial_member_can_remove_it_below_group_minimum() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let path = graph_path("events/partial-only-loop");
    let owner = node_id(912);
    let partial_id = instance_id(913);
    let mut document = GraphDocument::default();
    document
        .create_node(builtin_control_node(owner, "yssbi.control.loop"))
        .unwrap();
    bind_user_port(&mut document, owner, "next_source", partial_id);

    let patch = EditorGraphMutationDto::RemovePortInstance {
        address: PortAddress::instance(owner, PortKey::new("next_source").unwrap(), partial_id)
            .into(),
    }
    .into_patch(&path, &document, &registry)
    .unwrap();
    document.apply_patch(&patch).unwrap();
    assert!(document.port_bindings.is_empty());
}

#[test]
fn partial_member_does_not_consume_group_maximum() {
    let registry = builtin_registry_with_branch_group_max(1);
    let path = graph_path("events/partial-max");
    let owner = node_id(914);
    let partial_id = instance_id(915);
    let mut document = GraphDocument::default();
    document
        .create_node(builtin_control_node(owner, "yssbi.control.branch"))
        .unwrap();
    bind_user_port(&mut document, owner, "then_source", partial_id);

    let complete = EditorGraphMutationDto::AddPortInstance {
        node_id: owner,
        template: PortKey::new("result").unwrap(),
        order: None,
    }
    .into_patch(&path, &document, &registry)
    .unwrap();
    document.apply_patch(&complete).unwrap();
    assert!(
        EditorGraphMutationDto::AddPortInstance {
            node_id: owner,
            template: PortKey::new("else_source").unwrap(),
            order: None,
        }
        .into_patch(&path, &document, &registry)
        .is_err(),
        "the newly added complete member must consume max=1"
    );
}

#[test]
fn create_connect_and_add_port_allocate_identity_in_rust() {
    let registry = editor_mutation_registry();
    let path = graph_path("events/editor-mutation");
    let existing_id = node_id(911);
    let mut document = GraphDocument::default();
    document
        .create_node(editor_mutation_node(existing_id))
        .unwrap();

    let create_patch = EditorGraphMutationDto::CreateNode {
        descriptor: crate::node_system::catalog::NodeCreationDescriptor::Static {
            node_type_id: NodeTypeId::new("yssbi.test.editor_mutation").unwrap(),
        },
        position: NodePosition { x: 5.0, y: 8.0 },
        user_label: None,
    }
    .into_patch(&path, &document, &registry)
    .unwrap();
    let created_id = match &create_patch.operations[0] {
        GraphDocumentOperation::InsertNode { node } => node.id,
        operation => panic!("expected node insertion first, got {operation:?}"),
    };
    assert_ne!(created_id, existing_id);
    assert!(matches!(
        &create_patch.operations[1..],
        [GraphDocumentOperation::InsertPortBinding {
            address,
            binding: DynamicPortBinding::UserCreated { .. },
        }] if address.node_id == created_id
    ));
    document.apply_patch(&create_patch).unwrap();

    let add_patch = EditorGraphMutationDto::AddPortInstance {
        node_id: existing_id,
        template: PortKey::new("inputs").unwrap(),
        order: None,
    }
    .into_patch(&path, &document, &registry)
    .unwrap();
    let input = match &add_patch.operations[..] {
        [
            GraphDocumentOperation::InsertPortBinding {
                address,
                binding: DynamicPortBinding::UserCreated { .. },
            },
        ] => address.clone(),
        operations => panic!("unexpected add-port operations: {operations:?}"),
    };
    assert!(matches!(input.port, PortRef::Instance { .. }));
    document.apply_patch(&add_patch).unwrap();

    let connect_patch = EditorGraphMutationDto::Connect {
        output: declared(created_id, "output").into(),
        input: input.clone().into(),
        order: None,
    }
    .into_patch(&path, &document, &registry)
    .unwrap();
    let allocated_connection = match &connect_patch.operations[..] {
        [GraphDocumentOperation::InsertConnection { connection }] => connection,
        operations => panic!("unexpected connect operations: {operations:?}"),
    };
    assert_eq!(allocated_connection.output.node_id, created_id);
    assert_eq!(allocated_connection.input, input);
    assert!(!document.connections.contains_key(&allocated_connection.id));
}

#[test]
fn move_nodes_is_atomic_and_reversible() {
    let registry = editor_mutation_registry();
    let path = graph_path("events/move-nodes");
    let first = node_id(921);
    let second = node_id(922);
    let missing = node_id(923);
    let mut document = GraphDocument::default();
    document.create_node(editor_mutation_node(first)).unwrap();
    document.create_node(editor_mutation_node(second)).unwrap();
    let before = document.clone();

    let invalid = EditorGraphMutationDto::MoveNodes {
        positions: vec![
            NodePositionMutationDto {
                node_id: first,
                position: NodePosition { x: 13.0, y: 21.0 },
            },
            NodePositionMutationDto {
                node_id: missing,
                position: NodePosition { x: 34.0, y: 55.0 },
            },
        ],
    };
    assert!(invalid.into_patch(&path, &document, &registry).is_err());
    assert_graph_content_eq(&document, &before);

    let patch = EditorGraphMutationDto::MoveNodes {
        positions: vec![
            NodePositionMutationDto {
                node_id: first,
                position: NodePosition { x: 13.0, y: 21.0 },
            },
            NodePositionMutationDto {
                node_id: second,
                position: NodePosition { x: 34.0, y: 55.0 },
            },
        ],
    }
    .into_patch(&path, &document, &registry)
    .unwrap();
    assert_eq!(patch.operations.len(), 2);

    document.apply_patch(&patch).unwrap();
    assert_eq!(
        document.nodes[&first].position,
        NodePosition { x: 13.0, y: 21.0 }
    );
    assert_eq!(
        document.nodes[&second].position,
        NodePosition { x: 34.0, y: 55.0 }
    );
    document.apply_patch(&patch.inverse()).unwrap();
    assert_graph_content_eq(&document, &before);
}

#[test]
fn user_created_port_enforces_protocol_min_and_max() {
    let registry = editor_mutation_registry();
    let path = graph_path("events/user-created-port");
    let owner = node_id(931);
    let template = PortKey::new("inputs").unwrap();
    let first = PortAddress::instance(owner, template.clone(), instance_id(932));
    let mut document = GraphDocument::default();
    document.create_node(editor_mutation_node(owner)).unwrap();
    document
        .bind_port(
            first.clone(),
            DynamicPortBinding::UserCreated {
                order: OrderKey("a".into()),
            },
        )
        .unwrap();

    let add_patch = EditorGraphMutationDto::AddPortInstance {
        node_id: owner,
        template: template.clone(),
        order: Some(OrderKey("b".into())),
    }
    .into_patch(&path, &document, &registry)
    .unwrap();
    document.apply_patch(&add_patch).unwrap();
    assert!(
        EditorGraphMutationDto::AddPortInstance {
            node_id: owner,
            template: template.clone(),
            order: None,
        }
        .into_patch(&path, &document, &registry)
        .is_err()
    );

    let second = document
        .port_bindings
        .keys()
        .find(|address| **address != first)
        .cloned()
        .unwrap();
    let remove_patch = EditorGraphMutationDto::RemovePortInstance {
        address: second.into(),
    }
    .into_patch(&path, &document, &registry)
    .unwrap();
    document.apply_patch(&remove_patch).unwrap();
    assert!(
        EditorGraphMutationDto::RemovePortInstance {
            address: first.into(),
        }
        .into_patch(&path, &document, &registry)
        .is_err()
    );
}

#[test]
fn declared_port_address_needs_no_persisted_instance() {
    let first = node_id(1);
    let second = node_id(2);
    let mut document = GraphDocument::default();
    document.create_node(node(first)).unwrap();
    document.create_node(node(second)).unwrap();

    document
        .connect(declared(first, "output"), declared(second, "input"), None)
        .unwrap();

    assert!(document.port_bindings.is_empty());
    assert!(document.validate().is_ok());
}

#[test]
fn instance_address_requires_a_binding() {
    let first = node_id(1);
    let second = node_id(2);
    let mut document = GraphDocument::default();
    document.create_node(node(first)).unwrap();
    document.create_node(node(second)).unwrap();
    let input = PortAddress::instance(second, PortKey::new("fields").unwrap(), instance_id(10));

    assert!(matches!(
        document.connect(declared(first, "output"), input.clone(), None),
        Err(DocumentError::MissingPortBinding(address)) if address == input
    ));
    assert!(document.connections.is_empty());

    document.bind_port(input.clone(), binding()).unwrap();
    document
        .connect(declared(first, "output"), input, None)
        .unwrap();
}

#[test]
fn deleting_a_node_atomically_removes_owned_and_incident_data() {
    let first = node_id(1);
    let second = node_id(2);
    let mut document = GraphDocument::default();
    document.create_node(node(first)).unwrap();
    document.create_node(node(second)).unwrap();
    let input = PortAddress::instance(second, PortKey::new("fields").unwrap(), instance_id(10));
    document.bind_port(input.clone(), binding()).unwrap();
    document
        .set_literal(input.clone(), Some(json!(42)))
        .unwrap();
    document
        .connect(declared(first, "output"), input, None)
        .unwrap();

    document.delete_node(second).unwrap();

    assert!(!document.nodes.contains_key(&second));
    assert!(document.connections.is_empty());
    assert!(document.port_bindings.is_empty());
    assert!(document.input_states.is_empty());
    assert!(document.validate().is_ok());
}

#[test]
fn connections_override_but_do_not_discard_literals() {
    let first = node_id(1);
    let second = node_id(2);
    let mut document = GraphDocument::default();
    document.create_node(node(first)).unwrap();
    document.create_node(node(second)).unwrap();
    let input = declared(second, "input");
    document
        .set_literal(input.clone(), Some(json!(42)))
        .unwrap();
    let connection = document
        .connect(declared(first, "output"), input.clone(), None)
        .unwrap();

    assert_eq!(
        document.effective_input_binding(&input, Some(json!(0))),
        EffectiveInputBinding::Connections(vec![connection])
    );
    document.disconnect(connection).unwrap();
    assert_eq!(
        document.effective_input_binding(&input, Some(json!(0))),
        EffectiveInputBinding::Literal(json!(42))
    );
    document.set_literal(input.clone(), None).unwrap();
    assert_eq!(
        document.effective_input_binding(&input, Some(json!(0))),
        EffectiveInputBinding::ProtocolDefault(json!(0))
    );
}

#[test]
fn btree_maps_produce_stable_serialization() {
    let first = node_id(1);
    let second = node_id(2);
    let mut forward = GraphDocument::default();
    forward.create_node(node(first)).unwrap();
    forward.create_node(node(second)).unwrap();
    forward
        .set_literal(declared(second, "input"), Some(json!(42)))
        .unwrap();

    let mut reverse = GraphDocument::default();
    reverse.create_node(node(second)).unwrap();
    reverse.create_node(node(first)).unwrap();
    reverse
        .set_literal(declared(second, "input"), Some(json!(42)))
        .unwrap();

    let serialized = serde_json::to_string(&forward).unwrap();
    assert_eq!(serialized, serde_json::to_string(&reverse).unwrap());
    let restored: GraphDocument = serde_json::from_str(&serialized).unwrap();
    assert_eq!(restored.nodes, forward.nodes);
    assert_eq!(restored.input_states, forward.input_states);
}

#[test]
fn create_and_connect_transaction_undoes_and_redoes_with_original_identities() {
    let path = graph_path("events/history-test");
    let first = node_id(101);
    let second = node_id(102);
    let port_instance = instance_id(103);
    let connection_id = connection_id(104);
    let dynamic_input =
        PortAddress::instance(second, PortKey::new("fields").unwrap(), port_instance);
    let connection = DocumentConnection {
        id: connection_id,
        output: declared(first, "output"),
        input: dynamic_input.clone(),
        order: None,
    };
    let patch = GraphDocumentPatch::new(vec![
        GraphDocumentOperation::InsertNode { node: node(first) },
        GraphDocumentOperation::InsertNode { node: node(second) },
        GraphDocumentOperation::InsertPortBinding {
            address: dynamic_input.clone(),
            binding: binding(),
        },
        GraphDocumentOperation::InsertConnection {
            connection: connection.clone(),
        },
    ]);
    let transaction = ProjectHistoryTransaction::graph(
        operation_id(105),
        path.clone(),
        GraphRevision::INITIAL,
        patch,
    );
    let mut state = ProjectDocumentState::new(
        BTreeMap::from([(path.clone(), GraphDocument::default())]),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    let mut history = ProjectHistory::default();
    let original = state.graphs.get(&path).unwrap().clone();

    history.apply_transaction(&mut state, transaction).unwrap();
    let applied = state.graphs.get(&path).unwrap();
    assert!(applied.nodes.contains_key(&first));
    assert_eq!(applied.connections.get(&connection_id), Some(&connection));
    assert!(applied.port_bindings.contains_key(&dynamic_input));
    assert_eq!(applied.revision.get(), 1);
    assert_eq!(state.revision.get(), 1);
    let applied = applied.clone();

    history.undo(&mut state).unwrap();
    let undone = state.graphs.get(&path).unwrap();
    assert!(undone.nodes.is_empty());
    assert!(undone.connections.is_empty());
    assert!(undone.port_bindings.is_empty());
    assert_graph_content_eq(undone, &original);
    assert_eq!(undone.revision.get(), 2);
    assert_eq!(state.revision.get(), 2);

    history.redo(&mut state).unwrap();
    let redone = state.graphs.get(&path).unwrap();
    assert_eq!(redone.nodes.get(&first).unwrap().id, first);
    assert_eq!(redone.nodes.get(&second).unwrap().id, second);
    assert_eq!(redone.connections.get(&connection_id), Some(&connection));
    assert!(redone.port_bindings.contains_key(&PortAddress::instance(
        second,
        PortKey::new("fields").unwrap(),
        port_instance,
    )));
    assert_graph_content_eq(redone, &applied);
    assert_eq!(redone.revision.get(), 3);
    assert_eq!(state.revision.get(), 3);
}

#[test]
fn failed_multi_resource_transaction_is_atomic() {
    let first_path = graph_path("events/first");
    let second_path = graph_path("events/second");
    let valid_node = node(node_id(201));
    let missing_node = node_id(202);
    let first_patch = GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
        node: valid_node,
    }]);
    let invalid_connection = DocumentConnection {
        id: connection_id(203),
        output: declared(missing_node, "output"),
        input: declared(missing_node, "input"),
        order: None,
    };
    let second_patch = GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertConnection {
        connection: invalid_connection,
    }]);
    let transaction = ProjectHistoryTransaction::new(
        operation_id(204),
        vec![
            ResourcePatch::graph(first_path.clone(), GraphRevision::INITIAL, first_patch),
            ResourcePatch::graph(second_path.clone(), GraphRevision::INITIAL, second_patch),
        ],
    );
    let mut state = ProjectDocumentState::new(
        BTreeMap::from([
            (first_path, GraphDocument::default()),
            (second_path, GraphDocument::default()),
        ]),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    let before = state.clone();
    let mut history = ProjectHistory::default();

    assert!(matches!(
        history.apply_transaction(&mut state, transaction),
        Err(HistoryError::Patch { .. })
    ));
    assert_eq!(state, before);
    assert_eq!(history.undo_len(), 0);
    assert_eq!(history.redo_len(), 0);
}

#[test]
fn normal_mutation_after_undo_clears_redo_branch() {
    let path = graph_path("events/branch");
    let mut state = ProjectDocumentState::new(
        BTreeMap::from([(path.clone(), GraphDocument::default())]),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    let mut history = ProjectHistory::default();
    let first_patch = GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
        node: node(node_id(301)),
    }]);
    history
        .apply_transaction(
            &mut state,
            ProjectHistoryTransaction::graph(
                operation_id(302),
                path.clone(),
                GraphRevision::INITIAL,
                first_patch,
            ),
        )
        .unwrap();
    history.undo(&mut state).unwrap();
    assert!(history.can_redo());

    let branch_revision = state.graphs.get(&path).unwrap().revision;
    let branch_patch = GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
        node: node(node_id(303)),
    }]);
    history
        .apply_transaction(
            &mut state,
            ProjectHistoryTransaction::graph(
                operation_id(304),
                path,
                branch_revision,
                branch_patch,
            ),
        )
        .unwrap();

    assert!(!history.can_redo());
    assert!(matches!(
        history.redo(&mut state),
        Err(HistoryError::NothingToRedo)
    ));
    assert_eq!(state.revision.get(), 3);
}

#[test]
fn graph_patch_updates_node_content_reversibly() {
    let id = node_id(350);
    let before = node(id);
    let mut after = before.clone();
    after.position = NodePosition { x: 8.0, y: 13.0 };
    after.user_label = Some("updated".to_owned());
    after.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("value").unwrap(),
        json!(42),
    );
    let patch = GraphDocumentPatch::new(vec![GraphDocumentOperation::UpdateNode {
        before: before.clone(),
        after: after.clone(),
    }]);
    let mut document = GraphDocument::default();
    document.create_node(before.clone()).unwrap();

    document.apply_patch(&patch).unwrap();
    assert_eq!(document.nodes.get(&id), Some(&after));

    document.apply_patch(&patch.inverse()).unwrap();
    assert_eq!(document.nodes.get(&id), Some(&before));
    assert_eq!(document.revision.get(), 3);
}

#[test]
fn patch_kind_mismatch_is_rejected_without_mutation() {
    let path = graph_path("events/kind-mismatch");

    let function_patch = FunctionDocumentPatch::default();
    let resource_patch = ResourcePatch {
        resource: ResourceKey::Graph(path.clone()),
        before_revision: GraphRevision::INITIAL,
        after_revision: GraphRevision::new(1),
        forward: ResourceDocumentPatch::Function(function_patch.clone()),
        inverse: ResourceDocumentPatch::Function(function_patch.inverse()),
    };
    let transaction = ProjectHistoryTransaction::new(operation_id(361), vec![resource_patch]);
    let mut state = ProjectDocumentState::new(
        BTreeMap::from([(path, GraphDocument::default())]),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    let before = state.clone();
    let mut history = ProjectHistory::default();

    assert!(matches!(
        history.apply_transaction(&mut state, transaction),
        Err(HistoryError::ResourceKindMismatch {
            patch_kind: ResourceKind::Function,
            ..
        })
    ));
    assert_eq!(state, before);
    assert!(!history.can_undo());
    assert!(!history.can_redo());
}

#[test]
fn graph_patch_failure_leaves_document_and_revision_unchanged() {
    let existing = node(node_id(401));
    let mut document = GraphDocument::default();
    document.create_node(existing.clone()).unwrap();
    let before = document.clone();
    let patch = GraphDocumentPatch::new(vec![
        GraphDocumentOperation::InsertNode {
            node: node(node_id(402)),
        },
        GraphDocumentOperation::InsertNode { node: existing },
    ]);

    assert!(matches!(
        document.apply_patch(&patch),
        Err(DocumentError::DuplicateNode(_))
    ));
    assert_eq!(document, before);
}

#[test]
fn mutation_rejects_wrong_resource_without_changing_the_graph() {
    let path = graph_path("events/main");
    let requested = ResourceKey::Graph(graph_path("events/other"));
    let mut store = RevisionedGraphStore::new(path.clone(), GraphDocument::default());
    let before = store.document().clone();

    let result = store.apply_mutation(MutationRequest::new(
        requested.clone(),
        ResourceRevision::INITIAL,
        operation_id(500),
        GraphMutation::CreateNode {
            node: node(node_id(501)),
        },
    ));

    assert!(matches!(
        result,
        Err(MutationConflict::ResourceMismatch { requested: actual, store: expected })
            if actual == requested && expected == ResourceKey::Graph(path)
    ));
    assert_eq!(store.document(), &before);
}

#[test]
fn mutation_rejects_stale_revision_without_changing_the_graph() {
    let path = graph_path("events/main");
    let resource = ResourceKey::Graph(path.clone());
    let mut store = RevisionedGraphStore::new(path, GraphDocument::default());
    store
        .apply_mutation(MutationRequest::new(
            resource.clone(),
            ResourceRevision::INITIAL,
            operation_id(502),
            GraphMutation::CreateNode {
                node: node(node_id(503)),
            },
        ))
        .unwrap();
    let before = store.document().clone();

    let result = store.apply_mutation(MutationRequest::new(
        resource,
        ResourceRevision::INITIAL,
        operation_id(504),
        GraphMutation::CreateNode {
            node: node(node_id(505)),
        },
    ));

    assert!(matches!(
        result,
        Err(MutationConflict::StaleRevision {
            base_revision,
            current_revision,
        }) if base_revision == ResourceRevision::INITIAL
            && current_revision == ResourceRevision::new(1)
    ));
    assert_eq!(store.document(), &before);
}

#[test]
fn mutation_events_use_the_complete_graph_envelope() {
    let path = graph_path("events/main");
    let resource = ResourceKey::Graph(path.clone());
    let operation = operation_id(510);
    let mut store = RevisionedGraphStore::new(path.clone(), GraphDocument::default());

    let event = store
        .apply_mutation(MutationRequest::new(
            resource,
            ResourceRevision::INITIAL,
            operation,
            GraphMutation::CreateNode {
                node: node(node_id(511)),
            },
        ))
        .unwrap();

    assert_eq!(event.graph_path, path);
    assert_eq!(event.from_revision, ResourceRevision::INITIAL);
    assert_eq!(event.to_revision, ResourceRevision::new(1));
    assert_eq!(event.caused_by, Some(operation));
    assert_eq!(event.payload.operations.len(), 1);
}

#[test]
fn revision_gap_reports_the_missing_delta_range() {
    let event = GraphDeltaEvent {
        graph_path: graph_path("events/main"),
        from_revision: ResourceRevision::new(4),
        to_revision: ResourceRevision::new(5),
        caused_by: None,
        payload: GraphDocumentPatch::new(Vec::new()),
    };

    assert_eq!(
        detect_revision_gap(ResourceRevision::new(2), &event),
        Some(RevisionGap {
            expected_before_revision: ResourceRevision::new(2),
            actual_before_revision: ResourceRevision::new(4),
        })
    );
}

fn compilation_basis(path: &str, revision: GraphRevision) -> CompilationBasisToken {
    CompilationBasisToken::new(
        graph_path(path),
        revision,
        CompilationRegistryFingerprint::from_bytes([7; 32]),
        BTreeMap::from([(
            CompilationResourceKey::new("schema/source"),
            CompilationResourceVersion::new("v1"),
        )]),
    )
}

fn projected_member(path: &str, revision: GraphRevision, node_id: NodeId) -> ProjectedMemberRef {
    ProjectedMemberRef::new(
        compilation_basis(path, revision),
        node_id,
        PortKey::new("fields").unwrap(),
        DynamicMemberLocator::SchemaField {
            source: SchemaSourceIdentity("source".into()),
            field: SchemaFieldIdentity("field".into()),
        },
    )
}

fn authorization(member: ProjectedMemberRef) -> MaterializationAuthorization {
    MaterializationAuthorization::new(member, OrderKey("a".into()))
}

#[test]
fn compilation_basis_token_preserves_the_complete_resolver_basis() {
    let basis = compilation_basis("events/main", GraphRevision::new(3));
    let resource = CompilationResourceKey::new("schema/source");

    assert_eq!(basis.graph_path(), &graph_path("events/main"));
    assert_eq!(basis.graph_revision(), GraphRevision::new(3));
    assert_eq!(basis.registry_fingerprint().as_bytes(), &[7; 32]);
    assert_eq!(
        basis
            .resource_versions()
            .get(&resource)
            .map(|value| value.as_str()),
        Some("v1")
    );
}

#[test]
fn projected_member_rejects_a_stale_compilation_basis() {
    let path = graph_path("events/main");
    let source = node_id(530);
    let target = node_id(531);
    let mut document = GraphDocument::default();
    document.create_node(node(source)).unwrap();
    document.create_node(node(target)).unwrap();
    let mut store = RevisionedGraphStore::new(path.clone(), document);
    let before = store.document().clone();
    let member = projected_member("events/main", ResourceRevision::INITIAL, target);
    let authorization = authorization(member.clone());

    let result = store.apply_mutation(MutationRequest::new(
        ResourceKey::Graph(path),
        store.revision(),
        operation_id(532),
        GraphMutation::MaterializeProjectedMemberAndConnect {
            member,
            authorization,
            output: declared(source, "output"),
            order: None,
        },
    ));

    assert!(matches!(
        result,
        Err(MutationConflict::CompilationBasisStale { .. })
    ));
    assert_eq!(store.document(), &before);
}

#[test]
fn projected_member_rejects_authorization_for_another_member() {
    let path = graph_path("events/main");
    let source = node_id(535);
    let target = node_id(536);
    let mut document = GraphDocument::default();
    document.create_node(node(source)).unwrap();
    document.create_node(node(target)).unwrap();
    let mut store = RevisionedGraphStore::new(path.clone(), document);
    let member = projected_member("events/main", store.revision(), target);
    let other = ProjectedMemberRef::new(
        member.basis().clone(),
        target,
        member.template().clone(),
        DynamicMemberLocator::SchemaField {
            source: SchemaSourceIdentity("source".into()),
            field: SchemaFieldIdentity("forged".into()),
        },
    );
    let before = store.document().clone();

    let result = store.apply_mutation(MutationRequest::new(
        ResourceKey::Graph(path),
        store.revision(),
        operation_id(537),
        GraphMutation::MaterializeProjectedMemberAndConnect {
            member,
            authorization: authorization(other),
            output: declared(source, "output"),
            order: None,
        },
    ));

    assert!(matches!(
        result,
        Err(MutationConflict::MaterializationUnauthorized)
    ));
    assert_eq!(store.document(), &before);
}

#[test]
fn projected_member_materialization_and_connection_commit_atomically() {
    let path = graph_path("events/main");
    let resource = ResourceKey::Graph(path.clone());
    let source = node_id(540);
    let target = node_id(541);
    let mut document = GraphDocument::default();
    document.create_node(node(source)).unwrap();
    document.create_node(node(target)).unwrap();
    let mut store = RevisionedGraphStore::new(path, document);
    let before_revision = store.revision();
    let member = projected_member("events/main", before_revision, target);

    let event = store
        .apply_mutation(MutationRequest::new(
            resource.clone(),
            before_revision,
            operation_id(542),
            GraphMutation::MaterializeProjectedMemberAndConnect {
                authorization: authorization(member.clone()),
                member,
                output: declared(source, "output"),
                order: None,
            },
        ))
        .unwrap();

    assert_eq!(event.to_revision, before_revision.next());
    assert_eq!(store.document().port_bindings.len(), 1);
    assert_eq!(store.document().connections.len(), 1);
    let address = store.document().port_bindings.keys().next().unwrap();
    assert!(matches!(address.port, PortRef::Instance { .. }));
    assert_eq!(
        store.document().connections.values().next().unwrap().input,
        address.clone()
    );

    let invalid_source = node_id(543);
    let before_failed_request = store.document().clone();
    let member = projected_member("events/main", store.revision(), target);
    let result = store.apply_mutation(MutationRequest::new(
        resource,
        store.revision(),
        operation_id(544),
        GraphMutation::MaterializeProjectedMemberAndConnect {
            authorization: authorization(member.clone()),
            member,
            output: declared(invalid_source, "output"),
            order: None,
        },
    ));

    assert!(matches!(result, Err(MutationConflict::Document(_))));
    assert_eq!(store.document(), &before_failed_request);
}

fn function_key(value: &str) -> FunctionResourceKey {
    FunctionResourceKey(value.into())
}

fn variable_key(value: &str) -> VariableResourceKey {
    VariableResourceKey(value.into())
}

fn signature(parameter_name: &str) -> FunctionSignature {
    FunctionSignature {
        parameters: vec![FunctionParameter {
            id: FunctionParameterId("parameter-1".into()),
            name: parameter_name.into(),
            type_name: "number".into(),
        }],
        return_type: Some("number".into()),
    }
}

#[test]
fn function_signature_and_caller_graph_undo_as_one_project_transaction() {
    let graph_path = graph_path("events/caller");
    let function_key = function_key("functions/callee");
    let caller_node = node_id(620);
    let before_signature = signature("old");
    let after_signature = signature("new");
    let graph_patch = GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
        node: node(caller_node),
    }]);
    let function_patch =
        FunctionDocumentPatch::new(before_signature.clone(), after_signature.clone());
    let mut state = ProjectDocumentState::new(
        BTreeMap::from([(graph_path.clone(), GraphDocument::default())]),
        BTreeMap::from([(
            function_key.clone(),
            FunctionDocument::new(before_signature.clone()),
        )]),
        BTreeMap::new(),
    );
    let transaction = ProjectHistoryTransaction::new(
        operation_id(621),
        vec![
            ResourcePatch::function(
                function_key.clone(),
                ResourceRevision::INITIAL,
                function_patch,
            ),
            ResourcePatch::graph(graph_path.clone(), ResourceRevision::INITIAL, graph_patch),
        ],
    );
    let mut history = ProjectHistory::default();

    history.apply_transaction(&mut state, transaction).unwrap();
    assert_eq!(state.functions[&function_key].signature, after_signature);
    assert!(state.graphs[&graph_path].nodes.contains_key(&caller_node));

    history.undo(&mut state).unwrap();
    assert_eq!(state.functions[&function_key].signature, before_signature);
    assert!(state.graphs[&graph_path].nodes.is_empty());
    assert_eq!(state.functions[&function_key].revision.get(), 2);
    assert_eq!(state.graphs[&graph_path].revision.get(), 2);
    assert_eq!(state.revision.get(), 2);
}

#[test]
fn legacy_history_transaction_defaults_to_in_memory_until_save() {
    let transaction = ProjectHistoryTransaction::graph(
        operation_id(629),
        graph_path("events/legacy-history"),
        ResourceRevision::INITIAL,
        GraphDocumentPatch::new(Vec::new()),
    );
    let mut legacy = serde_json::to_value(&transaction).unwrap();
    legacy.as_object_mut().unwrap().remove("persistence");

    let decoded: ProjectHistoryTransaction = serde_json::from_value(legacy).unwrap();

    assert_eq!(
        decoded.persistence,
        HistoryPersistencePolicy::InMemoryUntilSave
    );
    assert_eq!(decoded.history_id, transaction.history_id);
    assert_eq!(decoded.caused_by, transaction.caused_by);
    assert_eq!(decoded.changes, transaction.changes);
    assert!(decoded.variable_effect_snapshots.is_none());
    assert!(decoded.graph_resource_move.is_none());
}

#[test]
fn variable_patch_is_reversible_and_monotonic() {
    let key = variable_key("variables/threshold");
    let patch = VariableDocumentPatch::new(Some(json!(10)), Some(json!(20)));
    let mut state = ProjectDocumentState::new(
        BTreeMap::new(),
        BTreeMap::new(),
        BTreeMap::from([(key.clone(), VariableDocument::new(json!(10)))]),
    );
    let transaction = ProjectHistoryTransaction::new(
        operation_id(630),
        vec![ResourcePatch::variable(
            key.clone(),
            ResourceRevision::INITIAL,
            patch,
        )],
    );
    let mut history = ProjectHistory::default();

    history.apply_transaction(&mut state, transaction).unwrap();
    assert_eq!(state.variables[&key].value, Some(json!(20)));
    history.undo(&mut state).unwrap();
    assert_eq!(state.variables[&key].value, Some(json!(10)));
    assert_eq!(state.variables[&key].revision.get(), 2);
}

#[test]
fn reload_replaces_project_state_and_clears_history() {
    let path = graph_path("events/reload");
    let mut state = ProjectDocumentState::new(
        BTreeMap::from([(path.clone(), GraphDocument::default())]),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    let mut history = ProjectHistory::default();
    history
        .apply_transaction(
            &mut state,
            ProjectHistoryTransaction::graph(
                operation_id(640),
                path,
                ResourceRevision::INITIAL,
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                    node: node(node_id(641)),
                }]),
            ),
        )
        .unwrap();

    let replacement = ProjectDocumentState::default();
    history.reload(&mut state, replacement.clone());

    assert_eq!(state, replacement);
    assert!(!history.can_undo());
    assert!(!history.can_redo());
}
