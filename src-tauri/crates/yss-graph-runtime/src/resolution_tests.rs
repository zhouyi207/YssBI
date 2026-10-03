use super::*;
use yss_data_contract::{DataValue, ValueType};
use yss_graph_document::{ConstantId, DocumentNode, GraphConstant, NodePosition};
use yss_graph_editor::NodePositionMutation;
use yss_graph_editor::projection::{EditorProjectionInput, build_editor_projection};
use yss_graph_resource_contract::{
    ColumnSchema, DataSchema, FunctionCatalogEntry, FunctionParameterContract, FunctionSignature,
    GraphResourceId,
};

fn runtime() -> GraphRuntimeState {
    GraphRuntimeState::from_components(
        GraphRuntimeEpoch::from_existing(1),
        super::tests::components(),
    )
    .unwrap()
}

#[test]
fn inventory_entries_remain_visible_but_unavailable_without_kernels() {
    let runtime = runtime();
    let mut catalog = runtime.localized_catalog_with_resources(&[], "zh-CN");
    runtime.annotate_catalog_availability(&mut catalog, |id| {
        id == "yssbi.statistics.linear.fit" || id.starts_with("yssbi.plot.")
    });
    let mut placeholders = 0;
    for item in &catalog.items {
        if item.node_type_id.starts_with("yssbi.statistics.") && item.ports.is_empty() {
            assert!(
                !item.available,
                "{} must not be creatable",
                item.node_type_id
            );
            placeholders += 1;
            assert!(
                item.documentation.is_some(),
                "{} requires help explaining its unavailable state",
                item.node_type_id
            );
        }
        if item.node_type_id.starts_with("yssbi.plot.") {
            assert!(
                !item.ports.is_empty(),
                "{} requires a concrete interface",
                item.node_type_id
            );
            assert!(
                item.available,
                "{} has an installed kernel",
                item.node_type_id
            );
        }
    }
    assert!(
        placeholders > 0,
        "the catalog still contains explicitly unavailable methods"
    );
    for (id, category, has_interface) in [
        ("yssbi.dataframe.labels", "dataframe.series", true),
        (
            "yssbi.dataframe.impute.single",
            "statistics.imputation",
            true,
        ),
        (
            "yssbi.dataframe.impute.multiple",
            "statistics.imputation",
            false,
        ),
        (
            "yssbi.dataframe.impute.mice",
            "statistics.imputation",
            false,
        ),
    ] {
        let item = catalog
            .items
            .iter()
            .find(|item| item.node_type_id.as_ref() == id)
            .unwrap();
        assert_eq!(item.category_id.as_ref(), category);
        assert!(!item.available);
        assert_eq!(!item.ports.is_empty(), has_interface, "{id}");
        assert!(item.documentation.is_some());
    }
    let encoding = catalog
        .items
        .iter()
        .find(|item| item.node_type_id.as_ref() == "yssbi.dataframe.encode")
        .unwrap();
    assert_eq!(encoding.category_id.as_ref(), "dataframe.series");
    assert!(!encoding.ports.is_empty());
    assert!(catalog.items.iter().any(|item| {
        item.node_type_id.as_ref() == "yssbi.statistics.linear.fit" && item.available
    }));
}

fn graph() -> GraphResourcePath {
    GraphResourcePath::new("events/Cache.yssbi-event").unwrap()
}

fn resources() -> ResourceCatalogSnapshot {
    ResourceCatalogSnapshot::new(BTreeMap::new(), BTreeMap::new())
}

#[test]
fn resource_titles_use_the_declared_resolved_parameter() {
    use yss_node_catalog::{
        CatalogResourcePath, ResourceBoundCreateArgs, build_builtin_node_system,
    };
    use yss_node_protocol::{ParameterEditorSpec, TypedValue};
    use yss_node_registry::{
        LeafImplementation, NodeRegistryBuilder, ProviderRegistration, RegisteredNode,
    };

    let builtin = build_builtin_node_system().unwrap();
    let mut protocol = builtin
        .registry
        .protocol(&"yssbi.dataframe.source.get".parse().unwrap())
        .unwrap()
        .clone();
    protocol.type_id = "tests.display.source".parse().unwrap();
    let mut parameters = protocol.parameters.groups[0].parameters.to_vec();
    let resource = parameters
        .iter_mut()
        .find(|parameter| parameter.key.as_str() == "dataframe")
        .unwrap();
    resource.default_value = Some(TypedValue {
        value_type: resource.value_type.clone(),
        value: DataValue::String("databases/selected".into()),
    });
    let mut note = resource.clone();
    note.key = "note".parse().unwrap();
    note.editor = ParameterEditorSpec::Text { multiline: false };
    note.default_value = None;
    note.constraints.clear();
    parameters.push(note);
    protocol.parameters.groups[0].parameters = parameters.into();
    let mut builder = NodeRegistryBuilder::new();
    yss_node_catalog::register_builtin_nodes(&mut builder).unwrap();
    let mut provider = ProviderRegistration::new("tests".parse().unwrap());
    provider.nodes = vec![RegisteredNode::leaf(
        Arc::new(protocol),
        LeafImplementation::new("tests.display.source"),
    )]
    .into();
    builder.register_provider(provider).unwrap();
    let runtime = GraphRuntimeState::from_components(
        GraphRuntimeEpoch::from_existing(1),
        GraphRuntimeComponents {
            registry: Arc::new(builder.freeze().unwrap()),
            catalog: builtin.catalog,
        },
    )
    .unwrap();
    let resources = ResourceCatalogSnapshot::new(
        BTreeMap::new(),
        BTreeMap::from([(
            GraphResourceId::new("databases/selected"),
            DataSchema { columns: vec![] },
        )]),
    );
    let entries = [("selected", "Selected data"), ("other", "Unrelated data")].map(|(id, name)| {
        CatalogResourceEntry {
            name: name.into(),
            node_type_id: "tests.display.source".parse().unwrap(),
            resource_path: CatalogResourcePath::new(format!("databases/{id}")),
            resource_revision: 1,
            create_args: ResourceBoundCreateArgs::Database,
            technical_terms: vec![],
        }
    });
    let mut document = GraphDocument::default();
    let id = node(&mut document, "tests.display.source", &[]);
    document.nodes.get_mut(&id).unwrap().user_label = Some("My source".into());
    for note in [None, Some("databases/other")] {
        if let Some(note) = note {
            document
                .nodes
                .get_mut(&id)
                .unwrap()
                .parameters
                .insert("note".parse().unwrap(), serde_json::json!(note));
        }
        let analysis = runtime.resolve_graph_document(
            &graph(),
            &document,
            &super::tests::basis(&runtime),
            &resources,
            &entries,
            "en-US",
        );
        let projection = build_editor_projection(EditorProjectionInput {
            graph_path: &graph(),
            document: &document,
            analysis: &analysis,
            registry_fingerprint: runtime.registry_fingerprint(),
        })
        .unwrap();
        assert_eq!(projection.nodes[0].display.title.as_ref(), "Selected data");
        assert_eq!(
            projection.nodes[0].display.user_label.as_deref(),
            Some("My source")
        );
        assert!(
            !document.nodes[&id]
                .parameters
                .contains_key(&"dataframe".parse().unwrap())
        );
    }
}

fn node(
    document: &mut GraphDocument,
    kind: &str,
    parameters: &[(&str, serde_json::Value)],
) -> NodeId {
    let id = NodeId::new();
    document.nodes.insert(
        id,
        DocumentNode {
            id,
            node_type: kind.parse().unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: parameters
                .iter()
                .map(|(key, value)| (key.parse().unwrap(), value.clone()))
                .collect(),
            user_label: None,
        },
    );
    id
}

fn constant_document() -> (GraphDocument, NodeId, ConstantId) {
    let mut document = GraphDocument::default();
    let id = ConstantId::new();
    document.constants.insert(
        id,
        GraphConstant {
            id,
            name: "Threshold".into(),
            data_type: ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
            data_value: DataValue::Integer(1),
            tabular: None,
            description: String::new(),
            tags: vec![],
        },
    );
    let node = node(
        &mut document,
        "yssbi.constant.get",
        &[("constant", serde_json::json!(id.to_string()))],
    );
    (document, node, id)
}

fn resolve(
    runtime: &GraphRuntimeState,
    document: &GraphDocument,
    resources: &ResourceCatalogSnapshot,
) -> GraphAnalysis {
    runtime.resolve_graph_document(
        &graph(),
        document,
        &super::tests::basis(runtime),
        resources,
        &[],
        "en-US",
    )
}

fn assert_reused(before: &GraphAnalysis, after: &GraphAnalysis, node: NodeId, expected: bool) {
    assert_eq!(
        Arc::ptr_eq(
            before
                .semantic_snapshot()
                .node(node)
                .unwrap()
                .constant
                .as_ref()
                .unwrap(),
            after
                .semantic_snapshot()
                .node(node)
                .unwrap()
                .constant
                .as_ref()
                .unwrap(),
        ),
        expected
    );
}

#[test]
fn connection_candidates_match_append_replace_and_type_rejections_without_editing() {
    use yss_graph_document::{ConnectionId, DocumentConnection};
    use yss_graph_editor::projection::{ConnectionDecision, ConnectionIntent};
    let runtime = runtime();
    let mut document = GraphDocument::default();
    let source = node(&mut document, "yssbi.constant.pi", &[]);
    let incumbent = node(&mut document, "yssbi.constant.pi", &[]);
    let target = node(&mut document, "yssbi.numeric.subtract", &[]);
    let viewer = node(&mut document, "yssbi.debug.view", &[]);
    let model = node(&mut document, "yssbi.statistics.linear.summary", &[]);
    let port = |id, key: &str| PortAddress::declared(id, key.parse().unwrap());
    let output = port(source, "value");
    let occupied = ConnectionId::new();
    let branch = ConnectionId::new();
    for (id, from, to) in [
        (occupied, port(incumbent, "value"), port(target, "left")),
        (branch, output.clone(), port(viewer, "data")),
    ] {
        document.connections.insert(
            id,
            DocumentConnection {
                id,
                output: from,
                input: to,
                order: None,
            },
        );
    }
    let original = document.clone();
    let analysis = resolve(&runtime, &document, &resources());
    let catalog = CatalogMutationValidationSnapshot {
        resources: BTreeMap::new(),
    };
    let candidates = runtime
        .connection_candidates(
            &graph(),
            &document,
            &output,
            ConnectionIntent::Connect,
            &catalog,
            &analysis,
        )
        .unwrap();
    let decision = |address| {
        candidates
            .candidates
            .iter()
            .find(|candidate| candidate.port == address)
            .unwrap()
            .decision
            .clone()
    };
    assert_eq!(decision(port(target, "right")), ConnectionDecision::Append);
    assert_eq!(
        decision(port(target, "left")),
        ConnectionDecision::Replace {
            displaced_connection_ids: vec![occupied]
        }
    );
    assert_eq!(
        decision(port(viewer, "data")),
        ConnectionDecision::Invalid {
            reason: "graph_connection_already_exists"
        }
    );
    assert_eq!(document, original, "a preview cannot mutate or claim ports");
    for input in [port(target, "right"), port(target, "left")] {
        let patch = runtime
            .plan_editor_mutation(
                &graph(),
                &document,
                EditorGraphMutation::Connect {
                    output: output.clone(),
                    input: input.clone(),
                    order: None,
                },
                &catalog,
                || analysis.clone(),
            )
            .unwrap();
        let mut committed = document.clone();
        apply_graph_document_patch(&mut committed, &patch).unwrap();
        assert!(
            committed.connections.contains_key(&branch),
            "output fan-out must survive"
        );
        assert!(
            committed
                .connections
                .values()
                .any(|connection| connection.input == input && connection.output == output)
        );
        assert_eq!(
            committed.connections.contains_key(&occupied),
            input != port(target, "left")
        );
    }
    let model_output = port(model, "result");
    let model_candidates = runtime
        .connection_candidates(
            &graph(),
            &document,
            &model_output,
            ConnectionIntent::Connect,
            &catalog,
            &analysis,
        )
        .unwrap();
    assert_eq!(
        model_candidates
            .candidates
            .iter()
            .find(|candidate| candidate.port == port(target, "right"))
            .unwrap()
            .decision,
        ConnectionDecision::Invalid {
            reason: "graph_connection_type_mismatch"
        }
    );
    assert_eq!(
        runtime
            .plan_editor_mutation(
                &graph(),
                &document,
                EditorGraphMutation::Connect {
                    output: port(target, "result"),
                    input: port(target, "right"),
                    order: None,
                },
                &catalog,
                || analysis.clone()
            )
            .unwrap_err()
            .code(),
        "graph_connection_same_node"
    );
}

#[test]
fn connection_candidates_for_moves_exclude_moved_links_from_replacement_preview() {
    use yss_graph_document::{ConnectionId, DocumentConnection};
    use yss_graph_editor::projection::{ConnectionDecision, ConnectionIntent};
    let runtime = runtime();
    let mut document = GraphDocument::default();
    let source = node(&mut document, "yssbi.constant.pi", &[]);
    let other = node(&mut document, "yssbi.constant.pi", &[]);
    let target = node(&mut document, "yssbi.numeric.subtract", &[]);
    let port = |id, key: &str| PortAddress::declared(id, key.parse().unwrap());
    let left = port(target, "left");
    let right = port(target, "right");
    let moved = ConnectionId::new();
    let displaced = ConnectionId::new();
    for (id, output, input) in [
        (moved, port(source, "value"), left.clone()),
        (displaced, port(other, "value"), right.clone()),
    ] {
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
    let analysis = resolve(&runtime, &document, &resources());
    let catalog = CatalogMutationValidationSnapshot {
        resources: BTreeMap::new(),
    };
    let candidates = runtime
        .connection_candidates(
            &graph(),
            &document,
            &left,
            ConnectionIntent::MoveConnections,
            &catalog,
            &analysis,
        )
        .unwrap();
    assert_eq!(
        candidates
            .candidates
            .iter()
            .find(|candidate| candidate.port == right)
            .unwrap()
            .decision,
        ConnectionDecision::Replace {
            displaced_connection_ids: vec![displaced]
        }
    );
    assert_eq!(
        candidates
            .candidates
            .iter()
            .find(|candidate| candidate.port == left)
            .unwrap()
            .decision,
        ConnectionDecision::Invalid {
            reason: "graph_connection_move_same_port"
        }
    );
    let patch = runtime
        .plan_editor_mutation(
            &graph(),
            &document,
            EditorGraphMutation::MoveConnections {
                source: left,
                target: right.clone(),
            },
            &catalog,
            || analysis.clone(),
        )
        .unwrap();
    apply_graph_document_patch(&mut document, &patch).unwrap();
    assert_eq!(document.connections.len(), 1);
    let connection = document.connections.values().next().unwrap();
    assert_eq!(connection.input, right);
    assert_eq!(connection.output, port(source, "value"));
}

#[test]
fn function_catalog_creation_preserves_authoritative_member_metadata() {
    use yss_data_contract::SemanticType;
    use yss_graph_document::{DynamicMemberLocator, FunctionParameterId};
    use yss_node_catalog::{CatalogResourcePath, NodeCreation, ResourceBoundCreateArgs};
    use yss_node_registry::{
        NodeRegistryBuilder, ProviderRegistration, RegisteredNode, StructuralNodeRole,
    };

    let builtin = yss_node_catalog::build_builtin_node_system().unwrap();
    let mut custom = builtin
        .registry
        .protocol(&"yssbi.project.function.call".parse().unwrap())
        .unwrap()
        .clone();
    custom.type_id = "tests.function.renamed".parse().unwrap();
    for port in &mut custom.interface.ports {
        port.key = match port.direction {
            PortDirection::Input => "input_members",
            PortDirection::Output => "output_members",
        }
        .parse()
        .unwrap();
    }
    let mut builder = NodeRegistryBuilder::new();
    yss_node_catalog::register_builtin_nodes(&mut builder).unwrap();
    let mut provider = ProviderRegistration::new("tests".parse().unwrap());
    provider.nodes = vec![RegisteredNode::structural(
        Arc::new(custom),
        StructuralNodeRole::Call,
    )]
    .into();
    builder.register_provider(provider).unwrap();
    let runtime = GraphRuntimeState::from_components(
        GraphRuntimeEpoch::from_existing(1),
        GraphRuntimeComponents {
            registry: Arc::new(builder.freeze().unwrap()),
            catalog: builtin.catalog,
        },
    )
    .unwrap();
    let function = GraphResourcePath::new("functions/Measure.yssbi-function").unwrap();
    let resource_path = CatalogResourcePath::new(function.as_str());
    let parameters = [
        ("flag", "Flag", SemanticType::Binary),
        ("amount", "Amount", SemanticType::Numeric),
    ];
    let catalog = ResourceCatalogSnapshot::new(
        BTreeMap::from([(
            function.clone(),
            FunctionCatalogEntry::new(FunctionSignature::new(
                parameters
                    .iter()
                    .map(|(id, name, semantic)| {
                        FunctionParameterContract::new(
                            FunctionParameterId::new(*id),
                            *name,
                            ValueType::Scalar(*semantic),
                        )
                    })
                    .collect(),
                Some(ValueType::Scalar(SemanticType::Numeric)),
            )),
        )]),
        BTreeMap::new(),
    );
    let entries = ["yssbi.project.function.call", "tests.function.renamed"].map(|node_type| {
        CatalogResourceEntry {
            name: "Measure".into(),
            node_type_id: node_type.parse().unwrap(),
            resource_path: resource_path.clone(),
            resource_revision: 7,
            create_args: ResourceBoundCreateArgs::FunctionGraph,
            technical_terms: vec![],
        }
    });
    let mutation_catalog = CatalogMutationValidationSnapshot {
        resources: BTreeMap::from([(
            resource_path.clone(),
            yss_graph_editor::CatalogMutationResource::Function {
                revision: entries[0].resource_revision,
                signature: catalog.function_signature(&function).unwrap().clone(),
            },
        )]),
    };
    let (mut document, constant, _) = constant_document();
    let subtract = node(&mut document, "yssbi.numeric.subtract", &[]);
    let before = document.clone();
    for (source, member, label, order) in [
        (
            PortAddress::declared(constant, "value".parse().unwrap()),
            "amount",
            "Amount",
            "00001",
        ),
        (
            PortAddress::declared(subtract, "left".parse().unwrap()),
            "return",
            "Result",
            "00000",
        ),
    ] {
        let compatible = runtime
            .compatible_catalog_with_resources(
                &graph(),
                &document,
                &source,
                &catalog,
                &entries,
                "en-US",
            )
            .unwrap();
        for entry in &entries {
            let descriptor = compatible
            .items
            .iter()
            .find(|item| item.resource_path.as_ref() == Some(&resource_path)
                && item.node_type_id.as_ref() == entry.node_type_id.as_str())
            .expect("function candidates must follow the declared resolver, including renamed templates")
            .creation
            .clone();
            let create = |descriptor| EditorGraphMutation::CreateNode {
                descriptor,
                position: NodePosition { x: 100.0, y: 0.0 },
                user_label: None,
                connect_from: Some(source.clone()),
            };
            let mut stale = descriptor.clone();
            let NodeCreation::ResourceBound {
                resource_revision, ..
            } = &mut stale
            else {
                panic!("function descriptor must be resource-bound")
            };
            *resource_revision -= 1;
            assert!(matches!(
                runtime.plan_editor_mutation(
                    &graph(),
                    &document,
                    create(stale),
                    &mutation_catalog,
                    || resolve(&runtime, &document, &catalog),
                ),
                Err(MutationConflict::CatalogResourceStale(_))
            ));
            let patch = runtime
                .plan_editor_mutation(
                    &graph(),
                    &document,
                    create(descriptor),
                    &mutation_catalog,
                    || resolve(&runtime, &document, &catalog),
                )
                .unwrap();
            assert_eq!(document, before);
            let mut created = document.clone();
            apply_graph_document_patch(&mut created, &patch).unwrap();
            assert_eq!(created.connections.len(), 1);
            assert_eq!(created.port_bindings.len(), 1);
            let (
                address,
                DynamicPortBinding::Resolved {
                    origin,
                    order: actual_order,
                    last_known,
                },
            ) = created.port_bindings.iter().next().unwrap()
            else {
                panic!("created function member must be resolved")
            };
            assert_eq!(
                origin,
                &DynamicMemberLocator::FunctionParameter {
                    function: function.clone(),
                    parameter: FunctionParameterId::new(member),
                }
            );
            assert_eq!(actual_order, &OrderKey::new(order));
            assert_eq!(last_known.label, label);
            let analysis = resolve(&runtime, &created, &catalog);
            let port = analysis
                .semantic_snapshot()
                .concrete_interface()
                .port(address)
                .unwrap();
            assert!(!port.orphan);
            assert_eq!(last_known.label, port.label.as_ref());
            assert_eq!(last_known.value_type.as_ref(), Some(&port.accepted_type));

            let mut persisted_orphan = created.clone();
            persisted_orphan.port_bindings.insert(
                address.clone(),
                DynamicPortBinding::Orphan {
                    origin: origin.clone(),
                    order: actual_order.clone(),
                    last_known: last_known.clone(),
                },
            );
            let missing_resources = resources();
            let scenarios = [
                (&created, &missing_resources, true),
                (&persisted_orphan, &catalog, false),
            ];
            let projections = scenarios.map(|(document, resources, _)| {
                let before = document.clone();
                let analysis = resolve(&runtime, document, resources);
                let projection = build_editor_projection(EditorProjectionInput {
                    graph_path: &graph(),
                    document,
                    analysis: &analysis,
                    registry_fingerprint: runtime.registry_fingerprint(),
                });
                assert_eq!(document, &before);
                projection
            });
            assert!(
                projections.iter().all(Result::is_ok),
                "missing/restored function members must project without rewriting bindings: {projections:?}"
            );
            for (projection, (_, _, orphan)) in projections.into_iter().zip(scenarios) {
                let projection = projection.unwrap();
                let port = projection
                    .nodes
                    .iter()
                    .flat_map(|node| node.ports.iter())
                    .find(|port| &port.address == address)
                    .unwrap();
                assert_eq!(port.orphan, orphan);
                assert_eq!(port.can_remove, orphan);
                assert_eq!(port.display.label.as_ref(), label);
                assert_eq!(projection.connections.len(), created.connections.len());
            }

            let mutations = [
                EditorGraphMutation::MoveConnections {
                    source: address.clone(),
                    target: match port.direction {
                        PortDirection::Input => {
                            PortAddress::declared(subtract, "left".parse().unwrap())
                        }
                        PortDirection::Output => {
                            PortAddress::declared(constant, "value".parse().unwrap())
                        }
                    },
                },
                EditorGraphMutation::InsertReroute {
                    connection_id: *created.connections.keys().next().unwrap(),
                    position: NodePosition { x: 50.0, y: 0.0 },
                },
            ];
            let restored_patches = mutations.clone().map(|mutation| {
                runtime.plan_editor_mutation(
                    &graph(),
                    &persisted_orphan,
                    mutation,
                    &mutation_catalog,
                    || resolve(&runtime, &persisted_orphan, &catalog),
                )
            });
            assert!(
                restored_patches.iter().all(Result::is_ok),
                "moves/reroutes must accept members restored by current resources: {restored_patches:?}"
            );
            for (index, restored_patch) in restored_patches.into_iter().enumerate() {
                let restored_patch = restored_patch.unwrap();
                let mut edited = persisted_orphan.clone();
                apply_graph_document_patch(&mut edited, &restored_patch).unwrap();
                assert_eq!(edited.connections.len(), index + 1);
                assert_eq!(edited.nodes.len(), persisted_orphan.nodes.len() + index);
                apply_graph_document_patch(&mut edited, &restored_patch.inverse()).unwrap();
                assert_eq!(edited, persisted_orphan);
            }
            for mutation in mutations {
                assert_eq!(
                    runtime
                        .plan_editor_mutation(
                            &graph(),
                            &created,
                            mutation,
                            &mutation_catalog,
                            || resolve(&runtime, &created, &missing_resources),
                        )
                        .unwrap_err()
                        .code(),
                    "graph_port_orphan"
                );
            }
            apply_graph_document_patch(&mut created, &patch.inverse()).unwrap();
            assert_eq!(created, before);
        }
    }
}

#[test]
fn layout_reuses_semantics_and_projects_current_display_but_constant_metadata_refreshes() {
    let runtime = runtime();
    let (mut document, node, id) = constant_document();
    let initial = resolve(&runtime, &document, &resources());
    document.nodes.get_mut(&node).unwrap().position.x = 240.0;
    document.nodes.get_mut(&node).unwrap().user_label = Some("New label".into());
    let moved = resolve(&runtime, &document, &resources());
    assert_reused(&initial, &moved, node, true);
    let projection = build_editor_projection(EditorProjectionInput {
        graph_path: &graph(),
        document: &document,
        analysis: &moved,
        registry_fingerprint: runtime.registry_fingerprint(),
    })
    .unwrap();
    assert_eq!(projection.nodes[0].position.x, 240.0);
    assert_eq!(
        projection.nodes[0].display.user_label.as_deref(),
        Some("New label")
    );
    document.constants.get_mut(&id).unwrap().name = "Renamed".into();
    let renamed = resolve(&runtime, &document, &resources());
    assert_reused(&moved, &renamed, node, false);
    assert_eq!(renamed.semantic_input_hash(), moved.semantic_input_hash());
    assert_eq!(
        renamed
            .semantic_snapshot()
            .node(node)
            .unwrap()
            .instance_title
            .as_deref(),
        Some("Renamed")
    );
    document.constants.get_mut(&id).unwrap().data_value = DataValue::Integer(2);
    let edited = resolve(&runtime, &document, &resources());
    assert_ne!(edited.semantic_input_hash(), renamed.semantic_input_hash());
    assert_eq!(edited, resolve(&self::runtime(), &document, &resources()));
}

#[test]
fn snapshot_rechecks_used_and_absent_resources_without_invalidating_unread_catalog_entries() {
    let runtime = runtime();
    let (mut document, constant, _) = constant_document();
    node(
        &mut document,
        "yssbi.dataframe.source.get",
        &[("dataframe", serde_json::json!("databases/used"))],
    );
    let catalog = |entries: &[(&str, ValueType)]| {
        ResourceCatalogSnapshot::new(
            BTreeMap::new(),
            entries
                .iter()
                .map(|(name, data_type)| {
                    (
                        GraphResourceId::new(*name),
                        DataSchema {
                            columns: vec![ColumnSchema {
                                semantic: None,
                                physical_type: None,
                                name: "amount".into(),
                                data_type: data_type.clone(),
                            }],
                        },
                    )
                })
                .collect(),
        )
    };
    let missing = resolve(&runtime, &document, &catalog(&[]));
    let unrelated = resolve(
        &runtime,
        &document,
        &catalog(&[(
            "databases/unread",
            ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
        )]),
    );
    assert_reused(&missing, &unrelated, constant, true);
    let present = resolve(
        &runtime,
        &document,
        &catalog(&[(
            "databases/used",
            ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
        )]),
    );
    assert_reused(&unrelated, &present, constant, false);
    let changed_catalog = catalog(&[(
        "databases/used",
        ValueType::Scalar(yss_data_contract::SemanticType::Text),
    )]);
    let changed = resolve(&runtime, &document, &changed_catalog);
    assert_reused(&present, &changed, constant, false);
    assert_eq!(
        changed,
        resolve(&self::runtime(), &document, &changed_catalog)
    );
    assert_eq!(missing, resolve(&runtime, &document, &catalog(&[])));
}

#[test]
fn function_labels_and_bodies_refresh_even_when_execution_identity_is_unchanged() {
    let runtime = runtime();
    let (mut document, constant, _) = constant_document();
    let path = GraphResourcePath::new("functions/Used.yssbi-function").unwrap();
    let call = node(
        &mut document,
        "yssbi.project.function.call",
        &[("target", serde_json::json!(path.as_str()))],
    );
    let catalog = |label: &str| {
        ResourceCatalogSnapshot::new(
            BTreeMap::from([(
                path.clone(),
                FunctionCatalogEntry::new(FunctionSignature::new(
                    vec![FunctionParameterContract::new(
                        yss_graph_document::FunctionParameterId::new("arg"),
                        label,
                        ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
                    )],
                    None,
                )),
            )]),
            BTreeMap::new(),
        )
    };
    let initial = resolve(&runtime, &document, &catalog("Before"));
    let renamed = resolve(&runtime, &document, &catalog("After"));
    assert_reused(&initial, &renamed, constant, false);
    assert_eq!(initial.semantic_input_hash(), renamed.semantic_input_hash());
    assert_eq!(
        renamed.semantic_snapshot().node(call).unwrap().ports[0]
            .label
            .as_ref(),
        "After"
    );
    let (body, body_node, id) = constant_document();
    let with_body = catalog("After").with_function_document(&path, body.clone());
    let first_body = resolve(&runtime, &document, &with_body);
    let mut edited_body = body;
    edited_body.constants.get_mut(&id).unwrap().name = "Body constant".into();
    let edited_catalog = catalog("After").with_function_document(&path, edited_body.clone());
    let new_body = resolve(&runtime, &document, &edited_catalog);
    assert_reused(&first_body, &new_body, constant, false);
    assert_eq!(
        first_body.semantic_input_hash(),
        new_body.semantic_input_hash()
    );
    assert_eq!(
        new_body,
        resolve(&self::runtime(), &document, &edited_catalog)
    );
    edited_body.nodes.get_mut(&body_node).unwrap().position.x = 50.0;
    let moved_catalog = catalog("After").with_function_document(&path, edited_body);
    assert_reused(
        &new_body,
        &resolve(&runtime, &document, &moved_catalog),
        constant,
        true,
    );
}

#[test]
fn kernel_capability_identity_invalidates_cached_analysis() {
    let runtime = runtime();
    let (document, node, _) = constant_document();
    let initial = resolve(&runtime, &document, &resources());
    let mut basis = super::tests::basis(&runtime);
    basis.kernel_fingerprint = [9; 32];
    let changed =
        runtime.resolve_graph_document(&graph(), &document, &basis, &resources(), &[], "en-US");
    assert_reused(&initial, &changed, node, false);
    assert_ne!(initial.semantic_input_hash(), changed.semantic_input_hash());
}

#[test]
fn snapshot_refreshes_diagnostic_connection_ids_excluded_from_execution_identity() {
    use yss_graph_analysis::GraphDiagnosticLocation;
    use yss_graph_document::{ConnectionId, DocumentConnection};
    let runtime = runtime();
    let (mut document, constant, _) = constant_document();
    let bad = ConnectionId::new();
    let source = PortAddress::declared(constant, "value".parse().unwrap());
    document.connections.insert(
        bad,
        DocumentConnection {
            id: bad,
            input: source.clone(),
            output: source,
            order: None,
        },
    );
    let initial = resolve(&runtime, &document, &resources());
    let replacement = ConnectionId::new();
    let mut connection = document.connections.remove(&bad).unwrap();
    connection.id = replacement;
    document.connections.insert(replacement, connection);
    let replaced = resolve(&runtime, &document, &resources());
    assert_eq!(
        initial.semantic_input_hash(),
        replaced.semantic_input_hash()
    );
    assert_reused(&initial, &replaced, constant, false);
    assert!(
        replaced
            .semantic_snapshot()
            .diagnostics()
            .iter()
            .any(|diagnostic| {
                diagnostic.primary == GraphDiagnosticLocation::Connection(replacement)
            })
    );
    assert!(
        replaced
            .semantic_snapshot()
            .diagnostics()
            .iter()
            .all(|diagnostic| { diagnostic.primary != GraphDiagnosticLocation::Connection(bad) })
    );
    assert_eq!(replaced, resolve(&self::runtime(), &document, &resources()));
}

#[test]
fn snapshot_rechecks_transitive_function_bodies() {
    let runtime = runtime();
    let (mut document, constant, _) = constant_document();
    let a = GraphResourcePath::new("functions/A.yssbi-function").unwrap();
    let b = GraphResourcePath::new("functions/B.yssbi-function").unwrap();
    node(
        &mut document,
        "yssbi.project.function.call",
        &[("target", serde_json::json!(a.as_str()))],
    );
    let mut a_body = GraphDocument::default();
    node(
        &mut a_body,
        "yssbi.project.function.entry",
        &[("function", serde_json::json!(a.as_str()))],
    );
    node(
        &mut a_body,
        "yssbi.project.function.call",
        &[("target", serde_json::json!(b.as_str()))],
    );
    let (mut b_body, _, id) = constant_document();
    node(
        &mut b_body,
        "yssbi.project.function.entry",
        &[("function", serde_json::json!(b.as_str()))],
    );
    let catalog = ResourceCatalogSnapshot::new(
        [a.clone(), b.clone()]
            .into_iter()
            .map(|path| {
                (
                    path,
                    FunctionCatalogEntry::new(FunctionSignature::new(vec![], None)),
                )
            })
            .collect(),
        BTreeMap::new(),
    )
    .with_function_document(&a, a_body);
    let missing = resolve(&runtime, &document, &catalog);
    let present = resolve(
        &runtime,
        &document,
        &catalog.clone().with_function_document(&b, b_body.clone()),
    );
    assert_reused(&missing, &present, constant, false);
    b_body.constants.get_mut(&id).unwrap().data_value = DataValue::Integer(7);
    let changed_catalog = catalog.with_function_document(&b, b_body);
    let changed = resolve(&runtime, &document, &changed_catalog);
    assert_reused(&present, &changed, constant, false);
    assert_ne!(present.semantic_input_hash(), changed.semantic_input_hash());
    assert_eq!(
        changed,
        resolve(&self::runtime(), &document, &changed_catalog)
    );
}

#[test]
fn document_only_mutations_do_not_request_semantics() {
    let runtime = runtime();
    let (document, node, _) = constant_document();
    let patch = runtime
        .plan_editor_mutation(
            &graph(),
            &document,
            EditorGraphMutation::MoveNodes {
                positions: vec![NodePositionMutation {
                    node_id: node,
                    position: NodePosition { x: 20.0, y: 0.0 },
                }],
            },
            &CatalogMutationValidationSnapshot {
                resources: BTreeMap::new(),
            },
            || panic!("move must not resolve"),
        )
        .unwrap();
    let mut moved = document;
    apply_graph_document_patch(&mut moved, &patch).unwrap();
    assert_eq!(moved.nodes[&node].position.x, 20.0);
}

#[test]
fn resolution_cache_evicts_old_graphs_without_changing_their_results() {
    let runtime = runtime();
    let (document, node, _) = constant_document();
    let initial = resolve(&runtime, &document, &resources());
    for index in 0..20 {
        runtime.resolve_graph_document(
            &GraphResourcePath::new(format!("events/Other{index}.yssbi-event")).unwrap(),
            &document,
            &super::tests::basis(&runtime),
            &resources(),
            &[],
            "en-US",
        );
    }
    let revisited = resolve(&runtime, &document, &resources());
    assert_reused(&initial, &revisited, node, false);
    assert_eq!(initial, revisited);
}

#[test]
#[ignore = "manual timing probe; run with --ignored --nocapture"]
fn benchmark_repeated_resolution_and_projection() {
    use std::hint::black_box;
    use std::time::Instant;
    use yss_graph_document::{ConnectionId, DocumentConnection};
    let runtime = runtime();
    let basis = super::tests::basis(&runtime);
    let graph = graph();
    let resources = ResourceCatalogSnapshot::new(
        BTreeMap::new(),
        BTreeMap::from([(
            GraphResourceId::new("databases/bench"),
            DataSchema {
                columns: (0..12)
                    .map(|index| ColumnSchema {
                        semantic: None,
                        physical_type: None,
                        name: format!("column{index}"),
                        data_type: ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
                    })
                    .collect(),
            },
        )]),
    );
    for (count, shape) in [100, 1000, 5000]
        .into_iter()
        .flat_map(|count| ["scalar", "dataframe"].map(|shape| (count, shape)))
    {
        let mut document = GraphDocument::default();
        let mut previous = None;
        for index in 0..count {
            if shape == "scalar" {
                node(&mut document, "yssbi.logic.not", &[]);
            } else if index % 10 == 0 {
                let source = node(
                    &mut document,
                    "yssbi.dataframe.source.get",
                    &[("dataframe", serde_json::json!("databases/bench"))],
                );
                previous = Some(PortAddress::declared(source, "dataframe".parse().unwrap()));
            } else {
                let consumer = node(
                    &mut document,
                    "yssbi.dataframe.limit",
                    &[("rows", serde_json::json!(10))],
                );
                let id = ConnectionId::new();
                document.connections.insert(
                    id,
                    DocumentConnection {
                        id,
                        input: PortAddress::declared(consumer, "source".parse().unwrap()),
                        output: previous.take().unwrap(),
                        order: None,
                    },
                );
                previous = Some(PortAddress::declared(consumer, "result".parse().unwrap()));
            }
        }
        // Warm type/Schema caches in both modes. Discard only the whole
        // snapshot to measure the additional benefit of snapshot reuse.
        runtime.resolve_graph_document(&graph, &document, &basis, &resources, &[], "en-US");
        for reuse in [false, true] {
            let samples = if cfg!(debug_assertions) && count == 5000 {
                4
            } else {
                20
            };
            let mut resolve_time = std::time::Duration::ZERO;
            let mut projection_time = std::time::Duration::ZERO;
            for _ in 0..samples {
                if !reuse {
                    let mut caches = runtime.semantic_caches.lock().unwrap();
                    let mut cache = caches.take(&graph);
                    cache.analysis = None;
                    let retired = caches.put(graph.clone(), cache);
                    drop(caches);
                    drop(retired);
                }
                let started = Instant::now();
                let analysis = runtime.resolve_graph_document(
                    &graph,
                    &document,
                    &basis,
                    &resources,
                    &[],
                    "en-US",
                );
                resolve_time += started.elapsed();
                let started = Instant::now();
                black_box(
                    build_editor_projection(EditorProjectionInput {
                        graph_path: &graph,
                        document: &document,
                        analysis: &analysis,
                        registry_fingerprint: runtime.registry_fingerprint(),
                    })
                    .unwrap(),
                );
                projection_time += started.elapsed();
            }
            println!(
                "shape={shape} nodes={count} snapshot_reuse={reuse} samples={samples} resolve_mean_ms={:.3} projection_mean_ms={:.3}",
                resolve_time.as_secs_f64() * 1000.0 / f64::from(samples),
                projection_time.as_secs_f64() * 1000.0 / f64::from(samples)
            );
        }
    }
}
