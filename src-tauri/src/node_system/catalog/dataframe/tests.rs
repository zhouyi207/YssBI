use super::{
    DATAFRAME_COLUMNS_RESOLVER, DATAFRAME_RESOURCE_SCHEMA_RESOLVER, build_provider_fragment,
};
use crate::data_contract::DataType;
use crate::graph_document::{
    ConnectionId, DocumentConnection, DocumentNode, DynamicMemberLocator, DynamicPortBinding,
    GraphDocument, GraphResourcePath, GraphRevision, LastKnownPortMetadata, NodeId, NodePosition,
    OrderKey, PortAddress, PortInstanceId, SchemaFieldIdentity, SchemaSourceIdentity,
};
use crate::node_system::analysis::{
    AnalysisResourceReads, AnalysisResourceResolver, ResolvedPortStatus, ResourceObservationSet,
    ResourceVersionSet,
};
use crate::node_system::catalog::builtin::build_builtin_node_system;
use crate::node_system::catalog::localization::Message;
use crate::node_system::compiler::ResourceSnapshot;
use crate::node_system::compiler::{
    CompileCancellationToken, CompileResult, DataframeColumnsResolver, GraphCompiler,
    InterfaceResolver, InterfaceResolverRequest, LoweredKernel, LoweringContext,
    NodeImplementation, ProjectedDynamicPortBinding, SchemaFact, SchemaFieldIdentityGuarantee,
    SchemaResolutionContext, SchemaResolutionError, SchemaResolver, SchemaResolverSet,
    ValidatedNodeConfig, build_builtin_interface_resolvers,
    builtin_function_interface_resolver_ids,
};
use crate::node_system::document::{
    GraphMutation, MutationRequest, ResourceKey as DocumentResourceKey, RevisionedGraphStore,
};
use crate::node_system::plan::{
    KernelHandle, RelationalExpression, RelationalLiteral, RelationalOperator,
    RelationalOperatorIndex, RelationalProjection, RelationalRename, ResourceId, ValueRef,
};
use crate::node_system::protocol::{
    InputConsumption, InterfaceResolverId, NodeProtocol, NodeTypeId, OutputProduction,
    ParameterConstraint, ParameterEditorSpec, ParameterKey, PortDirection, PortInstances, PortKey,
    RelationalScalarType, RenameExpr, SchemaColumnRef, SchemaExpr, SchemaField, SchemaFieldLineage,
    SchemaResolverId, TypeExpr, TypeId, TypeParameterId, Value, data_series_type,
    numeric_data_series_type,
};
use crate::node_system::runtime::build_builtin_kernel_registry;
use crate::project::OperationId;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

fn validated_config(
    registry: &crate::node_system::registry::NodeRegistry,
    protocol: &crate::node_system::protocol::NodeProtocol,
    parameters: BTreeMap<ParameterKey, serde_json::Value>,
) -> ValidatedNodeConfig {
    let validation = crate::node_system::protocol::validate_and_prepare_parameter_values(
        protocol,
        &parameters,
        |type_id, value| registry.prepare_nominal_parameter(type_id, value),
    );
    ValidatedNodeConfig::from_analysis(protocol, parameters, &validation.prepared_nominal)
        .expect("test configuration is analysis-valid")
}

struct EmptyResources;

impl ResourceSnapshot for EmptyResources {
    fn versions(&self) -> ResourceVersionSet {
        BTreeMap::new()
    }
}

#[derive(Default)]
struct EmptyAnalysisResources {
    reads: AnalysisResourceReads,
    observations: ResourceObservationSet,
}

impl AnalysisResourceResolver for EmptyAnalysisResources {
    fn resolve_function(
        &mut self,
        _: &crate::graph_document::GraphResourcePath,
    ) -> Result<
        crate::node_system::analysis::ResolvedFunction<'_>,
        crate::node_system::analysis::ResourceResolutionError,
    > {
        panic!("dataframe columns resolver must not read function resources")
    }

    fn resolve_variable(
        &mut self,
        _: &crate::variable::VariableId,
    ) -> Result<
        crate::node_system::analysis::ResolvedVariable<'_>,
        crate::node_system::analysis::ResourceResolutionError,
    > {
        panic!("dataframe columns resolver must not read variable resources")
    }

    fn resolve_database(
        &mut self,
        _: &str,
    ) -> Result<
        crate::node_system::analysis::ResolvedDatabase<'_>,
        crate::node_system::analysis::ResourceResolutionError,
    > {
        panic!("dataframe columns resolver must not read database resources")
    }

    fn reads(&self) -> &AnalysisResourceReads {
        &self.reads
    }

    fn observations(&self) -> &ResourceObservationSet {
        &self.observations
    }
}

struct DataframeSchemaResolver {
    fields: Box<[SchemaField]>,
}

impl SchemaResolver for DataframeSchemaResolver {
    fn resolve(
        &self,
        _: &mut SchemaResolutionContext<'_, '_>,
    ) -> Result<SchemaFact, SchemaResolutionError> {
        Ok(SchemaFact::new(
            SchemaExpr::Derived {
                resolver: SchemaResolverId::new(DATAFRAME_RESOURCE_SCHEMA_RESOLVER).unwrap(),
                dependencies: vec![],
            },
            self.fields.clone(),
        ))
    }
}

fn dataframe_node_id(value: u128) -> NodeId {
    NodeId::from_uuid(Uuid::from_u128(value))
}

fn dataframe_port(value: &str) -> PortKey {
    PortKey::new(value).unwrap()
}

fn stable_field(name: &str, scalar_type: RelationalScalarType) -> SchemaField {
    SchemaField {
        name: SchemaColumnRef(name.into()),
        scalar_type,
        lineage: Some(SchemaFieldLineage {
            source: "databases/main".into(),
            field: name.into(),
        }),
    }
}

fn dataframe_document(
    transform: Option<(&str, BTreeMap<ParameterKey, serde_json::Value>)>,
) -> GraphDocument {
    let source_id = dataframe_node_id(1);
    let decompose_id = dataframe_node_id(3);
    let mut nodes = BTreeMap::from([
        (
            source_id,
            DocumentNode {
                id: source_id,
                node_type: NodeTypeId::new("yssbi.dataframe.source.get").unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: BTreeMap::from([(
                    ParameterKey::new("dataframe").unwrap(),
                    serde_json::json!("databases/main"),
                )]),
                user_label: None,
            },
        ),
        (
            decompose_id,
            DocumentNode {
                id: decompose_id,
                node_type: NodeTypeId::new("yssbi.dataframe.decompose").unwrap(),
                position: NodePosition { x: 2.0, y: 0.0 },
                parameters: BTreeMap::new(),
                user_label: None,
            },
        ),
    ]);
    let mut connections = BTreeMap::new();

    if let Some((node_type, parameters)) = transform {
        let transform_id = dataframe_node_id(2);
        nodes.insert(
            transform_id,
            DocumentNode {
                id: transform_id,
                node_type: NodeTypeId::new(node_type).unwrap(),
                position: NodePosition { x: 1.0, y: 0.0 },
                parameters,
                user_label: None,
            },
        );
        insert_dataframe_connection(
            &mut connections,
            10,
            source_id,
            "dataframe",
            transform_id,
            "source",
        );
        insert_dataframe_connection(
            &mut connections,
            11,
            transform_id,
            "result",
            decompose_id,
            "dataframe",
        );
    } else {
        insert_dataframe_connection(
            &mut connections,
            10,
            source_id,
            "dataframe",
            decompose_id,
            "dataframe",
        );
    }

    GraphDocument {
        revision: GraphRevision::new(1),
        nodes,
        port_bindings: BTreeMap::new(),
        connections,
        input_states: BTreeMap::new(),
    }
}

fn insert_dataframe_connection(
    connections: &mut BTreeMap<ConnectionId, DocumentConnection>,
    connection: u128,
    output_node: NodeId,
    output_port: &str,
    input_node: NodeId,
    input_port: &str,
) {
    let id = ConnectionId::from_uuid(Uuid::from_u128(connection));
    connections.insert(
        id,
        DocumentConnection {
            id,
            output: PortAddress::declared(output_node, dataframe_port(output_port)),
            input: PortAddress::declared(input_node, dataframe_port(input_port)),
            order: None,
        },
    );
}

fn compile_dataframe_document(
    document: &GraphDocument,
    fields: Box<[SchemaField]>,
) -> CompileResult {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut schema_resolvers = SchemaResolverSet::new();
    schema_resolvers.insert(
        SchemaResolverId::new(DATAFRAME_RESOURCE_SCHEMA_RESOLVER).unwrap(),
        DataframeSchemaResolver { fields },
    );
    GraphCompiler::with_resolvers(
        &registry,
        &EmptyResources,
        schema_resolvers,
        build_builtin_interface_resolvers(),
    )
    .compile(document)
}

fn decompose_members(
    result: &CompileResult,
) -> Vec<&crate::node_system::compiler::ValidatedProjectedMember> {
    result
        .interface_projection
        .nodes
        .get(&dataframe_node_id(3))
        .into_iter()
        .flat_map(|projection| projection.available_members.iter())
        .filter(|member| member.template() == &dataframe_port("columns"))
        .collect()
}

fn editor_projection(
    document: &GraphDocument,
    result: &CompileResult,
) -> crate::node_system::analysis::EditorGraphProjectionDto {
    let builtin = build_builtin_node_system().unwrap();
    crate::node_system::analysis::build_editor_graph_projection(
        "events/dataframe-test.yssbi-event",
        document,
        &result.analysis,
        &result.outcome,
        builtin.registry.as_ref(),
        &builtin.catalog.localization("en-US"),
    )
    .unwrap()
}

fn decompose_resolved_ports(
    projection: &crate::node_system::analysis::EditorGraphProjectionDto,
) -> Vec<&crate::node_system::analysis::ResolvedPortDto> {
    projection
        .nodes
        .iter()
        .find(|node| node.node_id.as_ref() == dataframe_node_id(3).to_string())
        .expect("decompose node is projected")
        .ports
        .iter()
        .filter(|port| port.template_key.as_ref() == "columns")
        .collect()
}

#[test]
fn dataframe_columns_resolver_is_installed_in_builtin_set() {
    let resolvers = crate::node_system::compiler::build_builtin_interface_resolvers();
    let id = InterfaceResolverId::new(DATAFRAME_COLUMNS_RESOLVER).unwrap();
    assert!(resolvers.get(&id).is_some());
    for function_resolver in builtin_function_interface_resolver_ids() {
        assert!(resolvers.get(&function_resolver).is_some());
    }

    let fragment = build_provider_fragment().expect("dataframe built-in fixture must assemble");
    assert_eq!(fragment.interface_resolvers, vec![id.clone()]);
    let decompose = fragment
        .nodes
        .iter()
        .find(|node| node.protocol().type_id.as_str() == "yssbi.dataframe.decompose")
        .expect("decompose protocol");
    let columns = decompose
        .protocol()
        .interface
        .ports
        .iter()
        .find(|port| port.key.as_str() == "columns")
        .expect("decompose columns template");
    assert_eq!(columns.instances, PortInstances::Derived { resolver: id },);
}

#[test]
fn decompose_projects_final_upstream_schema() {
    let source_fields = || {
        vec![
            stable_field("customer_id", RelationalScalarType::Int64),
            stable_field("region", RelationalScalarType::String),
            stable_field("amount", RelationalScalarType::Float64),
        ]
        .into_boxed_slice()
    };
    let cases = [
        ("source", None, vec!["customer_id", "region", "amount"]),
        (
            "filter",
            Some((
                "yssbi.dataframe.filter.rows",
                BTreeMap::from([(
                    ParameterKey::new("predicate").unwrap(),
                    serde_json::json!({
                        "column": "amount",
                        "operator": "greaterThan",
                        "value": { "type": "integer", "value": "0" }
                    }),
                )]),
            )),
            vec!["customer_id", "region", "amount"],
        ),
        (
            "project",
            Some((
                "yssbi.dataframe.project",
                BTreeMap::from([(
                    ParameterKey::new("columns").unwrap(),
                    serde_json::json!(["amount", "customer_id"]),
                )]),
            )),
            vec!["amount", "customer_id"],
        ),
        (
            "rename",
            Some((
                "yssbi.dataframe.rename",
                BTreeMap::from([
                    (
                        ParameterKey::new("from").unwrap(),
                        serde_json::json!("customer_id"),
                    ),
                    (
                        ParameterKey::new("to").unwrap(),
                        serde_json::json!("account_id"),
                    ),
                ]),
            )),
            vec!["account_id", "region", "amount"],
        ),
    ];

    for (name, transform, expected) in cases {
        let document = dataframe_document(transform);
        let result = compile_dataframe_document(&document, source_fields());
        let codes = result
            .analysis
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<BTreeSet<_>>();
        let members = decompose_members(&result);
        let labels = members
            .iter()
            .map(|member| member.member().label.as_str())
            .collect::<Vec<_>>();
        let resolved_labels = result
            .analysis
            .resolved_schemas
            .get(&PortAddress::declared(
                dataframe_node_id(3),
                dataframe_port("dataframe"),
            ))
            .expect("decompose input schema is resolved")
            .fields
            .iter()
            .map(|field| field.name.0.as_ref())
            .collect::<Vec<_>>();

        assert!(
            !codes.contains("compiler.interface.resolver_missing"),
            "{name}: {:?}",
            result.analysis.diagnostics,
        );
        assert!(
            !codes.contains("compiler.interface.resolver_failed"),
            "{name}: {:?}",
            result.analysis.diagnostics,
        );
        assert_eq!(resolved_labels, expected, "{name} schema order");
        assert_eq!(labels, expected, "{name} projected labels");
        assert!(
            members
                .iter()
                .all(|member| { member.member().identity == SchemaFieldIdentityGuarantee::Stable })
        );

        if name == "rename" {
            let renamed = members
                .iter()
                .find(|member| member.member().label == "account_id")
                .expect("renamed field is projected");
            assert_eq!(
                renamed.member().locator,
                DynamicMemberLocator::SchemaField {
                    source: SchemaSourceIdentity::new("databases/main"),
                    field: SchemaFieldIdentity::new("customer_id"),
                },
            );
        }
    }
}

#[test]
fn decompose_projects_every_column_name_order_and_scalar_type() {
    let fields = vec![
        stable_field("active", RelationalScalarType::Boolean),
        stable_field("count", RelationalScalarType::Int64),
        stable_field("amount", RelationalScalarType::Float64),
        stable_field("name", RelationalScalarType::String),
        stable_field("day", RelationalScalarType::Date),
        stable_field("created", RelationalScalarType::DateTime),
    ];
    let document = dataframe_document(None);
    let result = compile_dataframe_document(&document, fields.into_boxed_slice());
    let projection = editor_projection(&document, &result);
    let ports = decompose_resolved_ports(&projection);

    assert_eq!(
        ports
            .iter()
            .map(|port| port.display.instance_label.as_deref().unwrap())
            .collect::<Vec<_>>(),
        vec!["active", "count", "amount", "name", "day", "created"],
    );
    assert_eq!(
        ports
            .iter()
            .map(|port| {
                port.resolved_type
                    .as_ref()
                    .and_then(|summary| summary.data_type.clone())
                    .unwrap()
            })
            .collect::<Vec<_>>(),
        vec![
            DataType::DataSeries(Box::new(DataType::Boolean)),
            DataType::DataSeries(Box::new(DataType::Int64)),
            DataType::DataSeries(Box::new(DataType::Float64)),
            DataType::DataSeries(Box::new(DataType::String)),
            DataType::DataSeries(Box::new(DataType::Date)),
            DataType::DataSeries(Box::new(DataType::Datetime)),
        ],
    );
}

#[test]
fn dataframe_field_type_unsupported_projects_any_and_port_diagnostic() {
    let document = dataframe_document(None);
    let result = compile_dataframe_document(
        &document,
        vec![stable_field("opaque", RelationalScalarType::Unknown)].into_boxed_slice(),
    );
    let projection = editor_projection(&document, &result);
    let port = decompose_resolved_ports(&projection)
        .into_iter()
        .next()
        .expect("unknown field remains projected");

    let summary = port.resolved_type.as_ref().unwrap();
    assert!(!summary.resolved);
    assert_eq!(summary.data_type, None);
    assert!(result.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "compiler.dataframe.field_type_unsupported"
            && diagnostic.arguments.get("column").map(Box::as_ref) == Some("opaque")
            && matches!(
                &diagnostic.primary,
                crate::node_system::analysis::DiagnosticLocation::Port(address)
                    if crate::node_system::analysis::PortAddressDto::from(address) == port.address
            )
    }));
}

#[test]
fn decompose_orphan_preserves_last_known_label_and_type() {
    let graph_path = GraphResourcePath::new("events/dataframe-orphan").unwrap();
    let decompose_id = dataframe_node_id(3);
    let consumer_id = dataframe_node_id(4);
    let customer_locator = DynamicMemberLocator::SchemaField {
        source: SchemaSourceIdentity::new("databases/main"),
        field: SchemaFieldIdentity::new("customer_id"),
    };
    let expected_metadata = LastKnownPortMetadata {
        label: "customer_id".into(),
        value_type: Some(data_series_type(TypeExpr::Concrete(
            TypeId::new("core.int64").unwrap(),
        ))),
    };
    let mut document = dataframe_document(None);
    document.nodes.insert(
        consumer_id,
        DocumentNode {
            id: consumer_id,
            node_type: NodeTypeId::new("yssbi.debug.view").unwrap(),
            position: NodePosition { x: 3.0, y: 0.0 },
            parameters: BTreeMap::new(),
            user_label: None,
        },
    );

    let resolved = compile_dataframe_document(
        &document,
        vec![stable_field("customer_id", RelationalScalarType::Int64)].into_boxed_slice(),
    );
    let candidate = decompose_members(&resolved)
        .into_iter()
        .find(|candidate| candidate.member().locator == customer_locator)
        .expect("current schema member is available for materialization");
    let (member, authorization) = candidate.authorize_materialization(
        graph_path.clone(),
        &resolved.interface_projection.basis,
        OrderKey::new("a"),
    );
    let mut store = RevisionedGraphStore::new(graph_path.clone(), document);
    store
        .apply_mutation(MutationRequest::new(
            DocumentResourceKey::Graph(graph_path),
            store.revision(),
            OperationId::from_uuid(Uuid::from_u128(21)),
            GraphMutation::MaterializeProjectedMemberAndConnect {
                member,
                authorization,
                output: PortAddress::declared(consumer_id, dataframe_port("data")),
                order: None,
            },
        ))
        .expect("validated current member materializes and connects atomically");
    let materialized = store.document();
    let (binding, persisted) = materialized
        .port_bindings
        .iter()
        .find(|(_, binding)| matches!(binding, DynamicPortBinding::Resolved { origin, .. } if origin == &customer_locator))
        .expect("production mutation persists the resolved member");
    assert_eq!(
        persisted,
        &DynamicPortBinding::Resolved {
            origin: customer_locator.clone(),
            order: OrderKey::new("a"),
            last_known: expected_metadata.clone(),
        },
    );
    let connection_id = materialized
        .connections
        .values()
        .find(|connection| connection.output == *binding)
        .expect("materialization preserves its connection")
        .id;

    let orphaned = compile_dataframe_document(materialized, Box::new([]));
    let node_projection = orphaned
        .interface_projection
        .nodes
        .get(&decompose_id)
        .expect("decompose projection");
    assert_eq!(
        node_projection.projected_bindings.get(binding),
        Some(&ProjectedDynamicPortBinding::Orphan {
            origin: customer_locator,
            order: OrderKey::new("a"),
            last_known: expected_metadata,
        }),
    );
    let projection = editor_projection(materialized, &orphaned);
    let orphan_port = decompose_resolved_ports(&projection)
        .into_iter()
        .find(|port| port.address == crate::node_system::analysis::PortAddressDto::from(binding))
        .expect("removed materialized member remains projected");
    assert!(orphan_port.orphan);
    assert_eq!(
        orphan_port.display.instance_label.as_deref(),
        Some("customer_id")
    );
    assert_eq!(
        orphan_port
            .resolved_type
            .as_ref()
            .and_then(|summary| summary.data_type.clone()),
        Some(DataType::DataSeries(Box::new(DataType::Int64))),
    );
    assert!(projection.connections.iter().any(|connection| {
        connection.connection_id.as_ref() == connection_id.to_string()
            && connection.output == crate::node_system::analysis::PortAddressDto::from(binding)
    }));
}

#[test]
fn dataframe_decompose_preserves_exact_dynamic_field_identity() {
    let decompose_id = dataframe_node_id(3);
    let binding = PortAddress::instance(
        decompose_id,
        dataframe_port("columns"),
        PortInstanceId::from_uuid(Uuid::from_u128(30)),
    );
    let customer_locator = DynamicMemberLocator::SchemaField {
        source: SchemaSourceIdentity::new("databases/main"),
        field: SchemaFieldIdentity::new("customer_id"),
    };
    let mut document = dataframe_document(None);
    document.port_bindings.insert(
        binding.clone(),
        DynamicPortBinding::Resolved {
            origin: customer_locator.clone(),
            order: OrderKey::new("a"),
            last_known: LastKnownPortMetadata::default(),
        },
    );
    let original_bindings = document.port_bindings.clone();

    let unchanged = compile_dataframe_document(
        &document,
        vec![stable_field("customer_id", RelationalScalarType::Int64)].into_boxed_slice(),
    );
    assert_eq!(document.port_bindings, original_bindings);
    let unchanged_interface = unchanged
        .analysis
        .resolved_interfaces
        .iter()
        .find(|interface| interface.node_id == decompose_id)
        .expect("decompose interface");
    assert!(
        unchanged_interface
            .ports
            .iter()
            .any(|port| { port.address == binding && port.status == ResolvedPortStatus::Resolved })
    );

    let removed = compile_dataframe_document(&document, Box::new([]));
    assert_eq!(document.port_bindings, original_bindings);
    let removed_projection = removed
        .interface_projection
        .nodes
        .get(&decompose_id)
        .expect("decompose projection");
    assert!(matches!(
        removed_projection.projected_bindings.get(&binding),
        Some(ProjectedDynamicPortBinding::Orphan { origin, .. }) if origin == &customer_locator
    ));

    let added = compile_dataframe_document(
        &document,
        vec![
            stable_field("customer_id", RelationalScalarType::Int64),
            stable_field("total", RelationalScalarType::Float64),
        ]
        .into_boxed_slice(),
    );
    assert_eq!(document.port_bindings, original_bindings);
    let total = decompose_members(&added)
        .into_iter()
        .find(|member| member.member().label == "total")
        .expect("new field is an unbound member");
    assert!(
        added
            .interface_projection
            .materialization_candidate(total.projection_address())
            .is_some()
    );

    let same_label = compile_dataframe_document(
        &document,
        vec![SchemaField {
            name: SchemaColumnRef("customer_id".into()),
            scalar_type: RelationalScalarType::Int64,
            lineage: Some(SchemaFieldLineage {
                source: "databases/archive".into(),
                field: "archived_customer_id".into(),
            }),
        }]
        .into_boxed_slice(),
    );
    assert_eq!(document.port_bindings, original_bindings);
    let same_label_projection = same_label
        .interface_projection
        .nodes
        .get(&decompose_id)
        .expect("decompose projection");
    assert!(matches!(
        same_label_projection.projected_bindings.get(&binding),
        Some(ProjectedDynamicPortBinding::Orphan { origin, .. }) if origin == &customer_locator
    ));
    let replacement = decompose_members(&same_label)
        .into_iter()
        .find(|member| member.member().label == "customer_id")
        .expect("different locator remains independently materializable");
    assert_ne!(replacement.member().locator, customer_locator);
    assert!(
        same_label
            .interface_projection
            .materialization_candidate(replacement.projection_address())
            .is_some()
    );
}

#[test]
fn dataframe_columns_resolver_rejects_duplicate_locators_deterministically() {
    let duplicate_fields = vec![
        stable_field("customer_id", RelationalScalarType::Int64),
        SchemaField {
            name: SchemaColumnRef("account_id".into()),
            scalar_type: RelationalScalarType::Int64,
            lineage: Some(SchemaFieldLineage {
                source: "databases/main".into(),
                field: "customer_id".into(),
            }),
        },
    ]
    .into_boxed_slice();
    let document = dataframe_document(None);
    let result = compile_dataframe_document(&document, duplicate_fields);
    let fragment = build_provider_fragment().expect("dataframe built-in fixture must assemble");
    let decompose = fragment
        .nodes
        .iter()
        .find(|node| node.protocol().type_id.as_str() == "yssbi.dataframe.decompose")
        .expect("decompose protocol");
    let protocol = decompose.protocol();
    let template = protocol
        .interface
        .ports
        .iter()
        .find(|port| port.key.as_str() == "columns")
        .expect("columns template");
    let mut resources = EmptyAnalysisResources::default();

    let error = DataframeColumnsResolver
        .resolve(InterfaceResolverRequest {
            basis: &result.analysis.basis,
            node_id: dataframe_node_id(3),
            template,
            protocol: &protocol,
            document: &document,
            resolved_schemas: &result.analysis.resolved_schemas,
            resources: &mut resources,
        })
        .expect_err("duplicate locator must fail instead of being deduplicated");

    assert_eq!(
        error.detail.as_ref(),
        "duplicate dataframe schema field locator 'databases/main/customer_id'",
    );
}

#[test]
fn dataframe_columns_without_lineage_are_snapshot_scoped() {
    let document = dataframe_document(None);
    let result = compile_dataframe_document(
        &document,
        vec![SchemaField {
            name: SchemaColumnRef("ephemeral".into()),
            scalar_type: RelationalScalarType::String,
            lineage: None,
        }]
        .into_boxed_slice(),
    );
    let member = decompose_members(&result)
        .into_iter()
        .next()
        .expect("schema field is projected");

    assert_eq!(
        member.member().identity,
        SchemaFieldIdentityGuarantee::SnapshotScoped,
    );
    assert_eq!(member.member().label, "ephemeral");
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
                    "nodes.yssbi.dataframe.rename.documentation",
                    "Renames the column identified by 'from' to 'to'.",
                ),
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
                    "nodes.yssbi.dataframe.rename.documentation",
                    "将“源列”指定的列重命名为“目标列”。",
                ),
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
            item.backend_search_text
                .iter()
                .any(|part| part.contains(search_term))
        );
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

fn dataframe_protocol(id: &str) -> NodeProtocol {
    build_provider_fragment()
        .expect("dataframe built-in fixture must assemble")
        .nodes
        .into_iter()
        .find(|node| node.protocol().type_id.as_str() == id)
        .unwrap_or_else(|| panic!("missing dataframe protocol '{id}'"))
        .protocol()
        .clone()
}

fn protocol_port_type(protocol: &NodeProtocol, key: &str) -> TypeExpr {
    protocol
        .interface
        .ports
        .iter()
        .find(|port| port.key.as_str() == key)
        .unwrap_or_else(|| panic!("missing port '{}:{}'", protocol.type_id, key))
        .value_type
        .clone()
}

#[test]
fn dataframe_series_protocols_use_canonical_data_series_contracts() {
    let boolean_series = data_series_type(TypeExpr::Concrete(TypeId::new("core.bool").unwrap()));
    let int64 = TypeExpr::Concrete(TypeId::new("core.int64").unwrap());
    let float64 = TypeExpr::Concrete(TypeId::new("core.float64").unwrap());
    let numeric = numeric_data_series_type();

    assert_eq!(
        protocol_port_type(&dataframe_protocol("yssbi.dataframe.filter"), "condition"),
        boolean_series,
    );
    for id in [
        "yssbi.dataframe.series.length",
        "yssbi.dataframe.series.count",
    ] {
        let protocol = dataframe_protocol(id);
        assert_eq!(protocol_port_type(&protocol, "value"), int64.clone());
    }
    assert_eq!(
        protocol_port_type(&dataframe_protocol("yssbi.dataframe.series.sum"), "series"),
        numeric.clone(),
    );
    assert_eq!(
        protocol_port_type(&dataframe_protocol("yssbi.dataframe.series.mean"), "series"),
        numeric.clone(),
    );
    assert_eq!(
        protocol_port_type(&dataframe_protocol("yssbi.dataframe.series.mean"), "value"),
        float64,
    );
    for id in [
        "yssbi.dataframe.series.standardize",
        "yssbi.dataframe.timeseries.difference",
        "yssbi.dataframe.panel.difference",
    ] {
        let protocol = dataframe_protocol(id);
        assert_eq!(protocol_port_type(&protocol, "series"), numeric.clone());
    }

    let lag = dataframe_protocol("yssbi.dataframe.timeseries.lag");
    let element = TypeParameterId::new("element").unwrap();
    assert_eq!(lag.interface.type_parameters.as_ref(), [element.clone()]);
    let generic_series = data_series_type(TypeExpr::Generic(element));
    assert_eq!(protocol_port_type(&lag, "series"), generic_series);
    assert_eq!(protocol_port_type(&lag, "result"), generic_series);

    let fragment = crate::node_system::catalog::core_nodes::build_provider_fragment().unwrap();
    let series_constructors = fragment
        .type_constructors
        .iter()
        .filter(|registration| {
            registration.id.as_str() == crate::node_system::protocol::DATA_SERIES_CONSTRUCTOR_ID
        })
        .collect::<Vec<_>>();
    assert_eq!(series_constructors.len(), 1);
    assert_eq!(series_constructors[0].arity, 1);
    assert_eq!(
        fragment
            .types
            .iter()
            .filter(|registration| registration.id.as_str()
                == crate::node_system::protocol::DATA_SERIES_CONSTRUCTOR_ID)
            .count(),
        0
    );
}

#[test]
fn dataframe_numeric_and_string_comparisons_remain_separate() {
    let numeric = numeric_data_series_type();
    let string = TypeExpr::Concrete(TypeId::new("core.string").unwrap());
    let string_series = data_series_type(string.clone());
    let boolean_series = data_series_type(TypeExpr::Concrete(TypeId::new("core.bool").unwrap()));

    for id in [
        "yssbi.dataframe.series.compare.equal",
        "yssbi.dataframe.series.compare.not_equal",
        "yssbi.dataframe.series.compare.greater",
        "yssbi.dataframe.series.compare.less",
        "yssbi.dataframe.series.compare.greater_equal",
        "yssbi.dataframe.series.compare.less_equal",
    ] {
        let protocol = dataframe_protocol(id);
        assert_eq!(protocol_port_type(&protocol, "left"), numeric.clone());
        assert_eq!(
            protocol_port_type(&protocol, "result"),
            boolean_series.clone()
        );
    }
    for id in [
        "yssbi.dataframe.series.compare.string.equal",
        "yssbi.dataframe.series.compare.string.not_equal",
    ] {
        let protocol = dataframe_protocol(id);
        assert_eq!(protocol_port_type(&protocol, "left"), string_series.clone());
        assert_eq!(
            protocol_port_type(&protocol, "right"),
            TypeExpr::Union(vec![string.clone(), string_series.clone()])
        );
        assert_eq!(
            protocol_port_type(&protocol, "result"),
            boolean_series.clone()
        );
    }
}

fn contains_unknown(value: &TypeExpr) -> bool {
    match value {
        TypeExpr::Unknown => true,
        TypeExpr::Applied { arguments, .. } | TypeExpr::Union(arguments) => {
            arguments.iter().any(contains_unknown)
        }
        TypeExpr::Concrete(_) | TypeExpr::Generic(_) => false,
    }
}

#[test]
fn dataframe_unknown_series_types_are_only_resolver_derived() {
    for node in build_provider_fragment()
        .expect("dataframe built-in fixture must assemble")
        .nodes
    {
        for port in &node.protocol().interface.ports {
            if contains_unknown(&port.value_type) {
                assert!(
                    matches!(port.instances, PortInstances::Derived { .. }),
                    "{}:{} uses Unknown without resolver-derived concretization",
                    node.protocol().type_id,
                    port.key
                );
            }
        }
    }
}
