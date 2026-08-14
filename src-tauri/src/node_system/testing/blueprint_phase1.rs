use crate::node_system::analysis::EditorGraphProjectionDto;
use crate::node_system::compiler::{
    LoweredKernel, LoweredNode, LoweringContext, LoweringError, NodeImplementation, NodeLowerer,
};
use crate::node_system::document::{
    ConnectionId, DocumentConnection, DocumentNode, EditorGraphMutationDto, GraphDocument,
    GraphDocumentOperation, GraphRevision, MutationConflict, MutationRequest, NodeId, NodePosition,
    OperationId, OrderKey, ParameterValues, PortAddress, PortAddressDto, ResourceKey,
    ResourceRevision,
};
use crate::node_system::plan::{CompiledParameterHandle, KernelHandle};
use crate::node_system::protocol::{
    CachePolicy, ConnectionsPerPort, Determinism, EffectSemantics, EvaluationPolicy,
    ExecutionSemantics, I18nKey, IconId, InputBindingSpec, LiteralPolicy, ManagedNodeRole,
    NodeCatalogProtocol, NodeCategoryId, NodeInterfaceProtocol, NodeProtocol, NodeScope,
    NodeStyleId, NodeTypeId, ParameterSchema, PortDirection, PortEditorSpec, PortInstances,
    PortKey, PortKind, PortSpec, ProviderId, Purity, TypeExpr, TypeId,
};
use crate::node_system::registry::{
    CategoryRegistration, I18nManifest, NodeRegistry, NodeRegistryBuilder, ProviderRegistration,
    RegisteredNode, StructuralNodeRole, TypeRegistration,
};
use crate::project::{
    GraphDocumentKind, GraphResourceDocument, GraphResourcePath, ProjectData, ProjectInstanceId,
    ProjectState,
};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

const SINGLE_SOURCE_INT: &str = "yssbi.testing.phase1.single_source_int";
const MULTI_SOURCE_INT: &str = "yssbi.testing.phase1.multi_source_int";
const BOUNDED_SOURCE_INT: &str = "yssbi.testing.phase1.bounded_source_int";
const MULTI_SOURCE_FLOAT: &str = "yssbi.testing.phase1.multi_source_float";
const SINK_INT: &str = "yssbi.testing.phase1.sink_int";
const MANAGED_EVENT: &str = "yssbi.testing.phase1.managed_event";
const INT_TYPE: &str = "yssbi.testing.phase1.int";
const FLOAT_TYPE: &str = "yssbi.testing.phase1.float";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase1ComplexMutation {
    ConnectReplacement,
    MoveConnections,
    DeleteNodes,
    DisconnectConnections,
    DisconnectPort,
    DisconnectNode,
}

pub(crate) const PHASE1_COMPLEX_MUTATIONS: [Phase1ComplexMutation; 6] = [
    Phase1ComplexMutation::ConnectReplacement,
    Phase1ComplexMutation::MoveConnections,
    Phase1ComplexMutation::DeleteNodes,
    Phase1ComplexMutation::DisconnectConnections,
    Phase1ComplexMutation::DisconnectPort,
    Phase1ComplexMutation::DisconnectNode,
];

impl Phase1ComplexMutation {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::ConnectReplacement => "connect replacement",
            Self::MoveConnections => "move connections",
            Self::DeleteNodes => "delete nodes",
            Self::DisconnectConnections => "disconnect connections",
            Self::DisconnectPort => "disconnect port",
            Self::DisconnectNode => "disconnect node",
        }
    }

    pub(crate) const fn validation_error_code(self) -> &'static str {
        match self {
            Self::ConnectReplacement => "graph_connection_type_mismatch",
            Self::MoveConnections => "graph_connection_limit_reached",
            Self::DeleteNodes => "graph_managed_node_delete_forbidden",
            Self::DisconnectConnections => "graph_connection_not_found",
            Self::DisconnectPort => "graph_port_not_found",
            Self::DisconnectNode => "graph_node_not_found",
        }
    }

    const fn ordinal(self) -> u128 {
        match self {
            Self::ConnectReplacement => 1,
            Self::MoveConnections => 2,
            Self::DeleteNodes => 3,
            Self::DisconnectConnections => 4,
            Self::DisconnectPort => 5,
            Self::DisconnectNode => 6,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Phase1AuthoritySnapshot {
    pub document: GraphDocument,
    pub serialized_document: Vec<u8>,
    pub revision: ResourceRevision,
    pub history_lengths: (usize, usize),
    pub projection: EditorGraphProjectionDto,
    pub publication: (String, u64, u64),
}

pub(crate) struct Phase1RegistryContract {
    pub bounded_maximum: Option<u16>,
    pub bounded_ordered: bool,
    pub ordered_maximum: Option<u16>,
    pub ordered_ordered: bool,
    pub registry_fingerprint: crate::node_system::registry::RegistryFingerprint,
    pub projection_registry_fingerprint: crate::node_system::registry::RegistryFingerprint,
    pub projected_bounded_maximum: Option<u32>,
    pub full_bounded_current: u32,
    pub full_bounded_maximum: Option<u32>,
    pub projected_ordered: bool,
}

pub(crate) struct BlueprintPhase1Fixture {
    pub state: ProjectState,
    pub project_instance_id: ProjectInstanceId,
    pub graph_path: GraphResourcePath,
    kind: Phase1ComplexMutation,
    connection_id_allocations: AtomicUsize,
    _project: crate::project::fixtures::TempProject,
}

impl BlueprintPhase1Fixture {
    pub(crate) fn new(kind: Phase1ComplexMutation) -> Self {
        let project = crate::project::fixtures::TempProject::activate(
            &format!("blueprint-phase1-{}", kind.ordinal()),
            ProjectData::new(),
        );
        let state = project.state().clone();
        state.project_store.write().unwrap().node_registry = Arc::new(test_registry());
        let graph_path = GraphResourcePath::new(format!(
            "events/BlueprintPhase1{}.yssbi-event",
            kind.ordinal()
        ))
        .unwrap();
        state
            .insert_graph(graph_path.clone(), graph_for(kind))
            .unwrap();
        let project_instance_id = state.capture_project_session().unwrap().instance_id;
        Self {
            state,
            project_instance_id,
            graph_path,
            kind,
            connection_id_allocations: AtomicUsize::new(0),
            _project: project,
        }
    }

    pub(crate) fn apply_editor_graph_mutation(
        &self,
        request: MutationRequest<EditorGraphMutationDto>,
    ) -> Result<crate::event::GraphMutationResultDto, MutationConflict> {
        let allocate = || {
            let ordinal = self
                .connection_id_allocations
                .fetch_add(1, Ordering::SeqCst)
                + 1;
            allocated_connection_id(self.kind, ordinal)
        };
        self.state
            .apply_editor_graph_mutation_with_allocator_for_test(
                &self.project_instance_id,
                &self.graph_path,
                "en-US",
                request,
                &allocate,
            )
    }

    pub(crate) fn connection_id_allocation_count(&self) -> usize {
        self.connection_id_allocations.load(Ordering::SeqCst)
    }

    pub(crate) fn success_request(
        &self,
        kind: Phase1ComplexMutation,
    ) -> MutationRequest<EditorGraphMutationDto> {
        self.assert_kind(kind);
        self.request(1, self.current_revision(), success_payload(kind))
    }

    pub(crate) fn validation_failure_request(
        &self,
        kind: Phase1ComplexMutation,
    ) -> MutationRequest<EditorGraphMutationDto> {
        self.assert_kind(kind);
        self.request(2, self.current_revision(), validation_failure_payload(kind))
    }

    pub(crate) fn stale_request(
        &self,
        kind: Phase1ComplexMutation,
    ) -> MutationRequest<EditorGraphMutationDto> {
        self.assert_kind(kind);
        self.request(3, self.current_revision().next(), success_payload(kind))
    }

    pub(crate) fn competing_requests(
        &self,
        kind: Phase1ComplexMutation,
    ) -> [MutationRequest<EditorGraphMutationDto>; 2] {
        self.assert_kind(kind);
        let revision = self.current_revision();
        [
            self.request(4, revision, success_payload(kind)),
            self.request(5, revision, success_payload(kind)),
        ]
    }

    pub(crate) fn empty_derived_disconnect_request(
        &self,
    ) -> MutationRequest<EditorGraphMutationDto> {
        let payload = match self.kind {
            Phase1ComplexMutation::DisconnectPort => EditorGraphMutationDto::DisconnectPort {
                address: dto(declared(node_id(0x402), "single_in")),
            },
            Phase1ComplexMutation::DisconnectNode => EditorGraphMutationDto::DisconnectNode {
                node_id: node_id(0x404),
            },
            _ => panic!("empty derived disconnect is only defined for port/node variants"),
        };
        self.request(7, self.current_revision(), payload)
    }

    pub(crate) fn duplicate_request(&self) -> MutationRequest<EditorGraphMutationDto> {
        self.assert_kind(Phase1ComplexMutation::ConnectReplacement);
        self.request(
            6,
            self.current_revision(),
            EditorGraphMutationDto::Connect {
                output: dto(declared(node_id(0x102), "out")),
                input: dto(declared(node_id(0x103), "single_in")),
                order: None,
            },
        )
    }

    pub(crate) fn expected_success_operations(
        &self,
        kind: Phase1ComplexMutation,
    ) -> Vec<GraphDocumentOperation> {
        self.assert_kind(kind);
        use GraphDocumentOperation::{InsertConnection, RemoveConnection, RemoveNode};
        match kind {
            Phase1ComplexMutation::ConnectReplacement => vec![
                RemoveConnection {
                    connection: connection(0xc101, 0x102, "out", 0x103, "single_in", None),
                },
                InsertConnection {
                    connection: DocumentConnection {
                        id: allocated_connection_id(kind, 1),
                        output: declared(node_id(0x101), "out"),
                        input: declared(node_id(0x103), "single_in"),
                        order: None,
                    },
                },
            ],
            Phase1ComplexMutation::MoveConnections => vec![
                RemoveConnection {
                    connection: connection(
                        0xc201,
                        0x201,
                        "out",
                        0x203,
                        "ordered_in",
                        Some("move-first"),
                    ),
                },
                RemoveConnection {
                    connection: connection(
                        0xc202,
                        0x201,
                        "out",
                        0x204,
                        "ordered_in",
                        Some("move-second"),
                    ),
                },
                InsertConnection {
                    connection: DocumentConnection {
                        id: allocated_connection_id(kind, 1),
                        output: declared(node_id(0x202), "out"),
                        input: declared(node_id(0x203), "ordered_in"),
                        order: Some(OrderKey("move-first".into())),
                    },
                },
                InsertConnection {
                    connection: DocumentConnection {
                        id: allocated_connection_id(kind, 2),
                        output: declared(node_id(0x202), "out"),
                        input: declared(node_id(0x204), "ordered_in"),
                        order: Some(OrderKey("move-second".into())),
                    },
                },
            ],
            Phase1ComplexMutation::DeleteNodes => vec![
                RemoveConnection {
                    connection: connection(0xc301, 0x301, "out", 0x302, "single_in", None),
                },
                RemoveNode {
                    node: document_node(0x301, SINGLE_SOURCE_INT),
                },
                RemoveNode {
                    node: document_node(0x302, SINK_INT),
                },
            ],
            Phase1ComplexMutation::DisconnectConnections
            | Phase1ComplexMutation::DisconnectPort
            | Phase1ComplexMutation::DisconnectNode => vec![
                RemoveConnection {
                    connection: connection(
                        0xc401,
                        0x401,
                        "out",
                        0x402,
                        "ordered_in",
                        Some("disconnect-first"),
                    ),
                },
                RemoveConnection {
                    connection: connection(
                        0xc402,
                        0x401,
                        "out",
                        0x403,
                        "ordered_in",
                        Some("disconnect-second"),
                    ),
                },
            ],
        }
    }

    pub(crate) fn document_connection_invariants_hold(&self) -> bool {
        let data = self.state.get_data().unwrap();
        let document = &data.graphs[&self.graph_path].document;
        let store = self.state.project_store.read().unwrap();
        let registry = &store.node_registry;
        for connection in document.connections.values() {
            let Some(output) = fixture_port(document, registry, &connection.output) else {
                return false;
            };
            let Some(input) = fixture_port(document, registry, &connection.input) else {
                return false;
            };
            if output.direction != PortDirection::Output
                || input.direction != PortDirection::Input
                || output.kind != input.kind
                || output.value_type != input.value_type
            {
                return false;
            }
            let ordered = matches!(
                input.connections,
                ConnectionsPerPort::Multiple { ordered: true, .. }
            );
            if ordered != connection.order.is_some() {
                return false;
            }
        }
        for node in document.nodes.values() {
            let Some(protocol) = registry.protocol(&node.node_type) else {
                return false;
            };
            for port in &protocol.interface.ports {
                let address = PortAddress::declared(node.id, port.key.clone());
                let current = document
                    .connections
                    .values()
                    .filter(|connection| match port.direction {
                        PortDirection::Output => connection.output == address,
                        PortDirection::Input => connection.input == address,
                    })
                    .count();
                let within_capacity = match port.connections {
                    ConnectionsPerPort::Single => current <= 1,
                    ConnectionsPerPort::Multiple { max: Some(max), .. } => {
                        current <= usize::from(max)
                    }
                    ConnectionsPerPort::Multiple { max: None, .. } => true,
                };
                if !within_capacity {
                    return false;
                }
            }
        }
        true
    }

    pub(crate) fn registry_contract(&self) -> Phase1RegistryContract {
        let store = self.state.project_store.read().unwrap();
        let registry = Arc::clone(&store.node_registry);
        drop(store);
        let bounded = registry
            .protocol(&node_type(BOUNDED_SOURCE_INT))
            .unwrap()
            .interface
            .ports
            .iter()
            .find(|port| port.key.as_str() == "out")
            .unwrap();
        let ordered = registry
            .protocol(&node_type(SINK_INT))
            .unwrap()
            .interface
            .ports
            .iter()
            .find(|port| port.key.as_str() == "ordered_in")
            .unwrap();
        let (bounded_maximum, bounded_ordered) = connection_contract(bounded.connections);
        let (ordered_maximum, ordered_ordered) = connection_contract(ordered.connections);
        let projection = self.projection();
        let projected_bounded = projected_port(&projection, 0x202, "out");
        let full_bounded = projected_port(&projection, 0x206, "out");
        let projected_ordered = projected_port(&projection, 0x203, "ordered_in");
        Phase1RegistryContract {
            bounded_maximum,
            bounded_ordered,
            ordered_maximum,
            ordered_ordered,
            registry_fingerprint: registry.fingerprint().clone(),
            projection_registry_fingerprint: projection.basis.registry_fingerprint.clone(),
            projected_bounded_maximum: projected_bounded.connections.maximum,
            full_bounded_current: full_bounded.connections.current,
            full_bounded_maximum: full_bounded.connections.maximum,
            projected_ordered: projected_ordered.connections.ordered,
        }
    }

    pub(crate) fn authority_snapshot(&self) -> Phase1AuthoritySnapshot {
        let data = self.state.get_data().unwrap();
        let document = data.graphs[&self.graph_path].document.clone();
        drop(data);
        Phase1AuthoritySnapshot {
            serialized_document: serde_json::to_vec(&document).unwrap(),
            revision: document.revision,
            document,
            history_lengths: self.state.history_lengths_for_test(),
            projection: self.projection(),
            publication: self.state.publication_state_for_test(),
        }
    }

    pub(crate) fn projection(&self) -> EditorGraphProjectionDto {
        self.state
            .graph_projection_for_project(&self.project_instance_id, &self.graph_path, "en-US")
            .unwrap()
    }

    pub(crate) fn resource_key(&self) -> ResourceKey {
        ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
            self.graph_path.as_str().into(),
        ))
    }

    fn current_revision(&self) -> ResourceRevision {
        self.state.get_data().unwrap().graphs[&self.graph_path]
            .document
            .revision
    }

    fn request(
        &self,
        flavor: u128,
        revision: ResourceRevision,
        payload: EditorGraphMutationDto,
    ) -> MutationRequest<EditorGraphMutationDto> {
        MutationRequest::new(
            self.resource_key(),
            revision,
            OperationId::from_uuid(uuid::Uuid::from_u128(
                0xb100_0000 + self.kind.ordinal() * 0x100 + flavor,
            )),
            payload,
        )
    }

    fn assert_kind(&self, kind: Phase1ComplexMutation) {
        assert_eq!(self.kind, kind, "fixture/request mutation kind mismatch");
    }
}

fn graph_for(kind: Phase1ComplexMutation) -> GraphResourceDocument {
    let mut graph = GraphResourceDocument::new(
        format!("Blueprint Phase 1 {}", kind.label()),
        GraphDocumentKind::Event,
    );
    match kind {
        Phase1ComplexMutation::ConnectReplacement => connect_graph(&mut graph),
        Phase1ComplexMutation::MoveConnections => move_graph(&mut graph),
        Phase1ComplexMutation::DeleteNodes => delete_graph(&mut graph),
        Phase1ComplexMutation::DisconnectConnections
        | Phase1ComplexMutation::DisconnectPort
        | Phase1ComplexMutation::DisconnectNode => disconnect_graph(&mut graph),
    }
    graph.document.revision = GraphRevision::INITIAL;
    graph
}

fn connect_graph(graph: &mut GraphResourceDocument) {
    insert_nodes(
        graph,
        &[
            (0x101, SINGLE_SOURCE_INT),
            (0x102, SINGLE_SOURCE_INT),
            (0x103, SINK_INT),
            (0x104, MULTI_SOURCE_FLOAT),
        ],
    );
    insert_connection(
        graph,
        connection(0xc101, 0x102, "out", 0x103, "single_in", None),
    );
}

fn move_graph(graph: &mut GraphResourceDocument) {
    insert_nodes(
        graph,
        &[
            (0x201, MULTI_SOURCE_INT),
            (0x202, BOUNDED_SOURCE_INT),
            (0x203, SINK_INT),
            (0x204, SINK_INT),
            (0x205, MULTI_SOURCE_INT),
            (0x206, BOUNDED_SOURCE_INT),
            (0x207, SINK_INT),
            (0x208, SINK_INT),
            (0x209, SINK_INT),
        ],
    );
    for connection in [
        connection(
            0xc201,
            0x201,
            "out",
            0x203,
            "ordered_in",
            Some("move-first"),
        ),
        connection(
            0xc202,
            0x201,
            "out",
            0x204,
            "ordered_in",
            Some("move-second"),
        ),
        connection(
            0xc203,
            0x206,
            "out",
            0x207,
            "ordered_in",
            Some("bounded-first"),
        ),
        connection(
            0xc204,
            0x206,
            "out",
            0x208,
            "ordered_in",
            Some("bounded-second"),
        ),
        connection(
            0xc205,
            0x205,
            "out",
            0x209,
            "ordered_in",
            Some("move-failure"),
        ),
    ] {
        insert_connection(graph, connection);
    }
}

fn delete_graph(graph: &mut GraphResourceDocument) {
    insert_nodes(
        graph,
        &[
            (0x301, SINGLE_SOURCE_INT),
            (0x302, SINK_INT),
            (0x303, MANAGED_EVENT),
        ],
    );
    insert_connection(
        graph,
        connection(0xc301, 0x301, "out", 0x302, "single_in", None),
    );
}

fn disconnect_graph(graph: &mut GraphResourceDocument) {
    insert_nodes(
        graph,
        &[
            (0x401, MULTI_SOURCE_INT),
            (0x402, SINK_INT),
            (0x403, SINK_INT),
            (0x404, SINK_INT),
        ],
    );
    insert_connection(
        graph,
        connection(
            0xc401,
            0x401,
            "out",
            0x402,
            "ordered_in",
            Some("disconnect-first"),
        ),
    );
    insert_connection(
        graph,
        connection(
            0xc402,
            0x401,
            "out",
            0x403,
            "ordered_in",
            Some("disconnect-second"),
        ),
    );
}

fn success_payload(kind: Phase1ComplexMutation) -> EditorGraphMutationDto {
    match kind {
        Phase1ComplexMutation::ConnectReplacement => EditorGraphMutationDto::Connect {
            output: dto(declared(node_id(0x101), "out")),
            input: dto(declared(node_id(0x103), "single_in")),
            order: None,
        },
        Phase1ComplexMutation::MoveConnections => EditorGraphMutationDto::MoveConnections {
            source: dto(declared(node_id(0x201), "out")),
            target: dto(declared(node_id(0x202), "out")),
        },
        Phase1ComplexMutation::DeleteNodes => EditorGraphMutationDto::DeleteNodes {
            node_ids: vec![node_id(0x302), node_id(0x301)],
        },
        Phase1ComplexMutation::DisconnectConnections => {
            EditorGraphMutationDto::DisconnectConnections {
                connection_ids: vec![connection_id(0xc402), connection_id(0xc401)],
            }
        }
        Phase1ComplexMutation::DisconnectPort => EditorGraphMutationDto::DisconnectPort {
            address: dto(declared(node_id(0x401), "out")),
        },
        Phase1ComplexMutation::DisconnectNode => EditorGraphMutationDto::DisconnectNode {
            node_id: node_id(0x401),
        },
    }
}

fn validation_failure_payload(kind: Phase1ComplexMutation) -> EditorGraphMutationDto {
    match kind {
        Phase1ComplexMutation::ConnectReplacement => EditorGraphMutationDto::Connect {
            output: dto(declared(node_id(0x104), "out")),
            input: dto(declared(node_id(0x103), "single_in")),
            order: None,
        },
        Phase1ComplexMutation::MoveConnections => EditorGraphMutationDto::MoveConnections {
            source: dto(declared(node_id(0x205), "out")),
            target: dto(declared(node_id(0x206), "out")),
        },
        Phase1ComplexMutation::DeleteNodes => EditorGraphMutationDto::DeleteNodes {
            node_ids: vec![node_id(0x301), node_id(0x303)],
        },
        Phase1ComplexMutation::DisconnectConnections => {
            EditorGraphMutationDto::DisconnectConnections {
                connection_ids: vec![connection_id(0xc4ff)],
            }
        }
        Phase1ComplexMutation::DisconnectPort => EditorGraphMutationDto::DisconnectPort {
            address: dto(declared(node_id(0x4ff), "missing")),
        },
        Phase1ComplexMutation::DisconnectNode => EditorGraphMutationDto::DisconnectNode {
            node_id: node_id(0x4ff),
        },
    }
}

fn test_registry() -> NodeRegistry {
    let int_type = TypeId::new(INT_TYPE).unwrap();
    let float_type = TypeId::new(FLOAT_TYPE).unwrap();
    let mut protocols = vec![
        protocol(
            SINGLE_SOURCE_INT,
            vec![port(
                SINGLE_SOURCE_INT,
                "out",
                PortDirection::Output,
                int_type.clone(),
                ConnectionsPerPort::Single,
            )],
            NodeScope::Any,
            None,
        ),
        protocol(
            MULTI_SOURCE_INT,
            vec![port(
                MULTI_SOURCE_INT,
                "out",
                PortDirection::Output,
                int_type.clone(),
                ConnectionsPerPort::Multiple {
                    max: None,
                    ordered: false,
                },
            )],
            NodeScope::Any,
            None,
        ),
        protocol(
            BOUNDED_SOURCE_INT,
            vec![port(
                BOUNDED_SOURCE_INT,
                "out",
                PortDirection::Output,
                int_type.clone(),
                ConnectionsPerPort::Multiple {
                    max: Some(2),
                    ordered: false,
                },
            )],
            NodeScope::Any,
            None,
        ),
        protocol(
            MULTI_SOURCE_FLOAT,
            vec![port(
                MULTI_SOURCE_FLOAT,
                "out",
                PortDirection::Output,
                float_type.clone(),
                ConnectionsPerPort::Multiple {
                    max: None,
                    ordered: false,
                },
            )],
            NodeScope::Any,
            None,
        ),
        protocol(
            SINK_INT,
            vec![
                port(
                    SINK_INT,
                    "single_in",
                    PortDirection::Input,
                    int_type.clone(),
                    ConnectionsPerPort::Single,
                ),
                port(
                    SINK_INT,
                    "ordered_in",
                    PortDirection::Input,
                    int_type,
                    ConnectionsPerPort::Multiple {
                        max: None,
                        ordered: true,
                    },
                ),
            ],
            NodeScope::Any,
            None,
        ),
        protocol(
            MANAGED_EVENT,
            vec![],
            NodeScope::Event,
            Some(ManagedNodeRole::EventBegin),
        ),
    ];
    protocols.sort_by(|left, right| left.type_id.cmp(&right.type_id));

    let mut keys = BTreeSet::new();
    keys.insert(i18n("categories.testing_phase1.title"));
    for type_id in [INT_TYPE, FLOAT_TYPE] {
        keys.insert(i18n(&format!("types.{type_id}.title")));
    }
    for protocol in &protocols {
        keys.insert(protocol.catalog.title_key.clone());
        keys.extend(
            protocol
                .interface
                .ports
                .iter()
                .map(|port| port.label_key.clone()),
        );
    }

    let nodes = protocols
        .into_iter()
        .map(|protocol| {
            if protocol.managed_role == Some(ManagedNodeRole::EventBegin) {
                RegisteredNode::structural(Arc::new(protocol), StructuralNodeRole::EventBegin)
            } else {
                let id = protocol.type_id.as_str().replace('.', "_");
                RegisteredNode::leaf(
                    Arc::new(protocol),
                    Arc::new(NodeImplementation::new(Phase1Lowerer {
                        kernel: KernelHandle::new(format!("testing.phase1.{id}")).unwrap(),
                        parameters: CompiledParameterHandle::new(format!(
                            "testing.phase1.parameters.{id}"
                        ))
                        .unwrap(),
                    })),
                )
            }
        })
        .collect::<Vec<_>>();

    let mut provider = ProviderRegistration::new(ProviderId::new("yssbi.testing.phase1").unwrap());
    provider.types = [
        TypeRegistration {
            id: TypeId::new(INT_TYPE).unwrap(),
            title_key: i18n(&format!("types.{INT_TYPE}.title")),
            classes: BTreeSet::new(),
        },
        TypeRegistration {
            id: TypeId::new(FLOAT_TYPE).unwrap(),
            title_key: i18n(&format!("types.{FLOAT_TYPE}.title")),
            classes: BTreeSet::new(),
        },
    ]
    .into();
    provider.categories = [CategoryRegistration {
        id: NodeCategoryId::new("testing_phase1").unwrap(),
        title_key: i18n("categories.testing_phase1.title"),
        parent: None,
        order: 0,
    }]
    .into();
    provider.i18n = I18nManifest { keys };
    provider.nodes = nodes.into_boxed_slice();

    let mut builder = NodeRegistryBuilder::new();
    builder.register_provider(provider).unwrap();
    builder.freeze().unwrap()
}

fn protocol(
    id: &str,
    ports: Vec<PortSpec>,
    scope: NodeScope,
    managed_role: Option<ManagedNodeRole>,
) -> NodeProtocol {
    NodeProtocol {
        type_id: node_type(id),
        catalog: NodeCatalogProtocol {
            title_key: i18n(&format!("nodes.{id}.title")),
            description_key: None,
            documentation_key: None,
            aliases_key: None,
            category_id: NodeCategoryId::new("testing_phase1").unwrap(),
            icon_id: IconId::new("testing.phase1").unwrap(),
            style_id: NodeStyleId::new("testing.phase1").unwrap(),
            hidden: false,
        },
        interface: NodeInterfaceProtocol::new(ports, vec![], vec![]).unwrap(),
        parameters: ParameterSchema::default(),
        execution: ExecutionSemantics {
            determinism: Determinism::Deterministic,
            purity: Purity::Pure,
            evaluation: EvaluationPolicy::DemandDriven,
            cache: CachePolicy::PerRun,
            effects: EffectSemantics::None,
            idempotent: false,
            retry: None,
        },
        scope,
        managed_role,
    }
}

fn port(
    node_type: &str,
    key: &str,
    direction: PortDirection,
    value_type: TypeId,
    connections: ConnectionsPerPort,
) -> PortSpec {
    PortSpec {
        key: PortKey::new(key).unwrap(),
        label_key: i18n(&format!("nodes.{node_type}.ports.{key}")),
        direction,
        kind: PortKind::Data,
        value_type: TypeExpr::Concrete(value_type),
        instances: PortInstances::Declared,
        connections,
        input_binding: (direction == PortDirection::Input).then_some(InputBindingSpec {
            literal_policy: LiteralPolicy::Forbidden,
            default_value: None,
        }),
        consumption: None,
        production: None,
        editor: PortEditorSpec::Default,
        schema: None,
    }
}

struct Phase1Lowerer {
    kernel: KernelHandle,
    parameters: CompiledParameterHandle,
}

impl NodeLowerer for Phase1Lowerer {
    fn lower(&self, _: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        Ok(LoweredNode {
            kernel: LoweredKernel::Native(self.kernel.clone()),
            parameters: self.parameters.clone(),
        })
    }
}

fn fixture_port<'a>(
    document: &GraphDocument,
    registry: &'a NodeRegistry,
    address: &PortAddress,
) -> Option<&'a PortSpec> {
    let node = document.nodes.get(&address.node_id)?;
    let protocol = registry.protocol(&node.node_type)?;
    let crate::node_system::document::PortRef::Declared { key } = &address.port else {
        return None;
    };
    protocol
        .interface
        .ports
        .iter()
        .find(|port| &port.key == key)
}

fn connection_contract(connections: ConnectionsPerPort) -> (Option<u16>, bool) {
    match connections {
        ConnectionsPerPort::Single => (Some(1), false),
        ConnectionsPerPort::Multiple { max, ordered } => (max, ordered),
    }
}

fn projected_port<'a>(
    projection: &'a EditorGraphProjectionDto,
    node: u128,
    template: &str,
) -> &'a crate::node_system::analysis::ResolvedPortDto {
    projection
        .nodes
        .iter()
        .find(|candidate| candidate.node_id.as_ref() == node_id(node).to_string())
        .and_then(|node| {
            node.ports
                .iter()
                .find(|port| port.template_key.as_ref() == template)
        })
        .unwrap()
}

fn insert_nodes(graph: &mut GraphResourceDocument, nodes: &[(u128, &str)]) {
    for &(id, node_type) in nodes {
        let node = document_node(id, node_type);
        graph.document.nodes.insert(node.id, node);
    }
}

fn insert_connection(graph: &mut GraphResourceDocument, connection: DocumentConnection) {
    graph.document.connections.insert(connection.id, connection);
}

fn document_node(id: u128, node_type_id: &str) -> DocumentNode {
    DocumentNode {
        id: node_id(id),
        node_type: node_type(node_type_id),
        position: NodePosition {
            x: id as f64,
            y: -(id as f64),
        },
        parameters: ParameterValues::new(),
        user_label: None,
    }
}

fn connection(
    id: u128,
    output_node: u128,
    output: &str,
    input_node: u128,
    input: &str,
    order: Option<&str>,
) -> DocumentConnection {
    DocumentConnection {
        id: connection_id(id),
        output: declared(node_id(output_node), output),
        input: declared(node_id(input_node), input),
        order: order.map(|value| OrderKey(value.into())),
    }
}

fn allocated_connection_id(kind: Phase1ComplexMutation, ordinal: usize) -> ConnectionId {
    connection_id(0xd000 + kind.ordinal() * 0x100 + ordinal as u128)
}

fn node_id(value: u128) -> NodeId {
    NodeId::from_uuid(uuid::Uuid::from_u128(value))
}

fn connection_id(value: u128) -> ConnectionId {
    ConnectionId::from_uuid(uuid::Uuid::from_u128(value))
}

fn node_type(value: &str) -> NodeTypeId {
    NodeTypeId::new(value).unwrap()
}

fn declared(node_id: NodeId, key: &str) -> PortAddress {
    PortAddress::declared(node_id, PortKey::new(key).unwrap())
}

fn dto(address: PortAddress) -> PortAddressDto {
    address.into()
}

fn i18n(value: &str) -> I18nKey {
    I18nKey::new(value).unwrap()
}
