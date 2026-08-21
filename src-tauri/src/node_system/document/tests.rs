use super::materialization::ProjectedMemberRef;
use super::mutation::{create_node_operations, validate_parameters};
use super::*;
use crate::node_system::analysis::ResourceVersionSet;
use crate::node_system::catalog::{
    build_builtin_node_system, builtin_bundle_parts_for_test, validate_builtin_bundle_for_test,
};
use crate::node_system::compatibility::{
    EditorMutationPortType, EditorMutationPortValidation, EditorMutationValidationSnapshot,
};
use crate::node_system::compiler::{
    GraphCompiler, LoweredNode, LoweringContext, LoweringError, NodeImplementation, NodeLowerer,
    ResourceSnapshot,
};
use crate::node_system::protocol::{
    CachePolicy, ConnectionsPerPort, Determinism, EffectSemantics, EvaluationPolicy, I18nKey,
    InputBindingSpec, LiteralPolicy, NodeCategoryId, NodeScope, NodeTypeId, ParameterKey,
    PortDirection, PortEditorSpec, PortInstances, PortKey, PortKind, PortSpec, ProviderId, Purity,
    TypeExpr, TypeId, TypeParameterId,
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
mod history;
mod insert_reroute;
mod lifecycle;
mod materialization;
mod mutation;
mod patch;
mod serialization;
mod subgraph;

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
        last_known: LastKnownPortMetadata::default(),
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

fn instance_identity_if_present(address: &PortAddress) -> Option<PortInstanceId> {
    match &address.port {
        PortRef::Instance { instance_id, .. } => Some(*instance_id),
        PortRef::Declared { .. } => None,
    }
}

fn create_connect_validation_snapshot(
    document: &GraphDocument,
    address: PortAddress,
    direction: PortDirection,
    port_type: EditorMutationPortType,
) -> EditorMutationValidationSnapshot {
    EditorMutationValidationSnapshot {
        graph_revision: document.revision,
        ports: BTreeMap::from([(
            address,
            EditorMutationPortValidation {
                direction,
                kind: PortKind::Data,
                orphan: false,
                port_type,
            },
        )]),
    }
}

fn compatibility_snapshot() -> crate::project::CatalogMutationValidationSnapshot {
    crate::project::CatalogMutationValidationSnapshot {
        project_instance_id: crate::project::ProjectInstanceId::new(),
        authority_generation: 0,
        resources: BTreeMap::new(),
    }
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
        PortDirection::Input,
        DynamicMemberLocator::SchemaField {
            source: SchemaSourceIdentity("source".into()),
            field: SchemaFieldIdentity("field".into()),
        },
        LastKnownPortMetadata {
            label: "Field".into(),
            value_type: Some(TypeExpr::Unknown),
        },
    )
}

fn authorization(member: ProjectedMemberRef) -> MaterializationAuthorization {
    MaterializationAuthorization::new(member, OrderKey("a".into()))
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
            type_name: "Number".into(),
        }],
        return_type: Some("Number".into()),
    }
}

fn worksheet_document(
    revision: ResourceRevision,
    database_id: &str,
    chart_type: &str,
) -> crate::project::WorksheetDocument {
    let mut document = crate::project::WorksheetDocument::new(database_id);
    document.revision = revision;
    document.chart_type = chart_type.into();
    document.encodings = crate::project::WorksheetEncodings {
        x: Some("region".into()),
        y: Some("revenue".into()),
    };
    document
}

fn worksheet_state(database_id: &str, chart_type: &str) -> WorksheetDocumentState {
    WorksheetDocumentState {
        database_id: database_id.into(),
        chart_type: chart_type.into(),
        encodings: crate::project::WorksheetEncodings {
            x: Some("region".into()),
            y: Some("revenue".into()),
        },
    }
}
