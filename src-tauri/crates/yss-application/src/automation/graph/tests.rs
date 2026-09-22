use super::*;
use crate::session::{ApplicationSessionEpoch, ApplicationSessionSlot};
use std::time::Duration;

struct Fixture {
    directory: std::path::PathBuf,
    application: Option<ApplicationState>,
    context: CapabilityInvocationContext,
    path: String,
    document: GraphDocument,
    revision: u64,
    dataset: String,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.application.take();
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
impl Fixture {
    fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("yss-assistant-graph-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let project = Arc::new(yss_project::ProjectState::new());
        let created = project
            .create_project_transaction("Assistant", &directory.join("project"), OperationId::new())
            .unwrap();
        project
            .activate_project_from_path(&created.metadata_path)
            .unwrap();

        let candidate = crate::session::build_current_project_candidate(
            ApplicationSessionEpoch::INITIAL,
            project,
            [],
            &crate::session::NodeComponents::builtins().unwrap(),
        )
        .unwrap();
        let application = ApplicationState::new(Arc::new(ApplicationSessionSlot::new(
            crate::session::NodeComponents::builtins().unwrap(),
        )));
        application.install_candidate(candidate).unwrap();
        let session = application.capture_session().unwrap();
        let instance = session.project_instance_id().clone();
        let context = CapabilityInvocationContext::new(
            PrincipalId::try_new("user-1").unwrap(),
            HarnessSessionId::try_new("session-1").unwrap(),
            CapabilityInvocationId::try_new("capability-1").unwrap(),
            ProjectSessionBinding::new(instance.clone(), session.project_session_id().clone()),
        );
        application
            .create_graph_resource(
                instance.clone(),
                "Main".into(),
                yss_graph_document::GraphResourceKind::Event,
                OperationId::new(),
            )
            .unwrap();
        let path = session
            .project()
            .get_data()
            .unwrap()
            .graphs
            .keys()
            .next()
            .unwrap()
            .as_str()
            .to_owned();
        let csv = directory.join("sample.csv");
        std::fs::write(&csv, "x,label\n1,a\n2,b\n3,c\n").unwrap();
        let dataset = application
            .load_database_for_application(
                instance,
                OperationId::new(),
                yss_database_contract::DatabaseImportSource::Csv {
                    path: csv.to_string_lossy().into(),
                    delimiter: ',',
                    has_header: true,
                    infer_schema_length: Some(10),
                },
            )
            .unwrap()
            .data
            .id;
        drop(session);
        Self {
            directory,
            application: Some(application),
            context,
            path,
            document: GraphDocument::default(),
            revision: 0,
            dataset,
        }
    }
    fn action(
        &mut self,
        request: AutomationCapabilityRequest,
    ) -> Result<AutomationCapabilityResult, CapabilityFailure> {
        let result = self
            .application
            .as_ref()
            .unwrap()
            .invoke_automation_capability(
                self.context.clone(),
                request,
                &CapabilityControl::new(CancellationToken::default(), Duration::from_secs(15)),
            )?;
        let captured = self
            .application
            .as_ref()
            .unwrap()
            .capture_session()
            .unwrap();
        let snapshot = captured
            .project()
            .read_graph_editing(
                captured.project_instance_id(),
                &GraphResourcePath::new(&self.path).unwrap(),
            )
            .unwrap();
        self.document = (*snapshot.document).clone();
        self.revision = snapshot.state.version.revision.get();
        Ok(result)
    }
    fn inspect(&mut self) -> GraphInspection {
        let AutomationCapabilityResult::GraphInspection(graph) = self
            .action(AutomationCapabilityRequest::InspectGraph(
                InspectGraphRequest {
                    graph_path: self.path.clone(),
                },
            ))
            .unwrap()
        else {
            panic!("graph result")
        };
        graph
    }
    fn edit_request(&self, operations: Vec<GraphEditOperation>) -> AutomationCapabilityRequest {
        AutomationCapabilityRequest::ApplyGraphEdit(ApplyGraphEditRequest {
            graph_path: self.path.clone(),
            base_revision: self.revision,
            graph_hash: graph_hash(&self.document).unwrap(),
            client_key: uuid::Uuid::new_v4().to_string(),
            locale: "en-US".into(),
            operations,
        })
    }
    fn edit(&mut self, operations: Vec<GraphEditOperation>) -> GraphEditReceipt {
        let AutomationCapabilityResult::GraphEditReceipt(receipt) =
            self.action(self.edit_request(operations)).unwrap()
        else {
            panic!("edit receipt")
        };
        receipt
    }
}
fn node(kind: &str, alias: &str) -> GraphEditOperation {
    GraphEditOperation::CreateNode {
        client_id: Some(alias.into()),
        node_type_id: kind.into(),
        resource_path: None,
        x: 100.,
        y: 100.,
        user_label: None,
    }
}
fn port(node: &str, key: &str) -> GraphEditPortRef {
    GraphEditPortRef::Declared {
        node_id: node.into(),
        port_key: key.into(),
    }
}
fn connect(output: GraphEditPortRef, input: GraphEditPortRef) -> GraphEditOperation {
    GraphEditOperation::Connect {
        output,
        input,
        order: None,
    }
}

#[test]
fn assistant_discovers_edits_and_saves_graphs_without_open_editor_panels() {
    let mut f = Fixture::new();
    let captured = f.application.as_ref().unwrap().capture_session().unwrap();
    let project = captured.project_instance_id().clone();
    let path = GraphResourcePath::new(&f.path).unwrap();
    let saved_path = f.directory.join("project").join(&f.path);
    let saved = std::fs::read(&saved_path).unwrap();
    f.application
        .as_ref()
        .unwrap()
        .unload_graph_resource(project.clone(), path.clone(), 100, None)
        .unwrap();
    assert!(!captured.project().has_resident_graph(&path).unwrap());

    let AutomationCapabilityResult::ProjectInspection(inspection) = f
        .application
        .as_ref()
        .unwrap()
        .invoke_automation_capability(
            f.context.clone(),
            AutomationCapabilityRequest::InspectProject(InspectProjectRequest {}),
            &CapabilityControl::new(CancellationToken::default(), Duration::from_secs(15)),
        )
        .unwrap()
    else {
        panic!("project inspection");
    };
    assert!(inspection.resources.iter().any(|resource| {
        resource.kind == ProjectResourceKindInspection::Graph && resource.resource_id == f.path
    }));
    assert!(inspection.resources.iter().any(|resource| {
        resource.kind == ProjectResourceKindInspection::Database
            && resource.resource_id == f.dataset
    }));
    assert!(!captured.project().has_resident_graph(&path).unwrap());

    f.inspect();
    let receipt = f.edit(vec![node("yssbi.numeric.multiply", "created")]);
    let application = f.application.as_ref().unwrap();
    let snapshot = captured
        .project()
        .read_graph_editing(&project, &path)
        .unwrap();
    assert!(!snapshot.state.dirty && snapshot.state.can_undo);
    let saved_edit = std::fs::read(&saved_path).unwrap();
    assert_ne!(saved_edit, saved);
    let undo = application
        .change_graph_history(
            GraphEditRequest {
                project_instance_id: project.clone(),
                graph_path: path.clone(),
                locale: "en-US".into(),
                operation_id: OperationId::new(),
                version: snapshot.state.version,
            },
            false,
        )
        .unwrap();
    assert!(undo.update.document.nodes.is_empty());
    assert!(undo.editing.dirty && undo.editing.can_redo);
    let redo = application
        .change_graph_history(
            GraphEditRequest {
                project_instance_id: project.clone(),
                graph_path: path.clone(),
                locale: "en-US".into(),
                operation_id: OperationId::new(),
                version: undo.editing.version,
            },
            true,
        )
        .unwrap();
    assert!(!redo.editing.dirty);
    assert_eq!(std::fs::read(&saved_path).unwrap(), saved_edit);
    captured.project().unload_graph_resource(&path).unwrap();
    assert!(!captured.project().has_resident_graph(&path).unwrap());
    let opened = f
        .application
        .as_ref()
        .unwrap()
        .open_graph(crate::graph::open::OpenGraphRequest::new(
            project, path, 0, "en-US",
        ))
        .unwrap();
    let created =
        NodeId::from_uuid(uuid::Uuid::parse_str(&receipt.created_nodes["created"]).unwrap());
    assert!(opened.document().nodes.contains_key(&created));
    assert!(!opened.editing().dirty);
    assert_eq!(std::fs::read(saved_path).unwrap(), saved_edit);
}

#[test]
fn assistant_autosave_failure_preserves_the_previous_document_history_and_file() {
    let mut f = Fixture::new();
    f.inspect();
    let application = f.application.as_ref().unwrap().clone();
    let captured = application.capture_session().unwrap();
    let project = captured.project_instance_id().clone();
    let path = GraphResourcePath::new(&f.path).unwrap();
    let initial = captured
        .project()
        .read_graph_editing(&project, &path)
        .unwrap();
    let manual = application
        .edit_graph(
            GraphEditRequest {
                project_instance_id: project.clone(),
                graph_path: path.clone(),
                locale: "en-US".into(),
                version: initial.state.version,
                operation_id: OperationId::new(),
            },
            EditorGraphMutation::CreateNode {
                descriptor: yss_node_catalog::NodeCreation::Static {
                    node_type_id: "yssbi.numeric.multiply".parse().unwrap(),
                },
                position: NodePosition { x: 0., y: 0. },
                user_label: None,
                connect_from: None,
            },
        )
        .unwrap();
    assert!(manual.editing.dirty && manual.editing.can_undo);
    f.inspect();
    let request = f.edit_request(vec![node("yssbi.numeric.multiply", "assistant")]);
    let before = captured
        .project()
        .read_graph_editing(&project, &path)
        .unwrap();
    let file = f.directory.join("project").join(&f.path);
    let saved = std::fs::read(&file).unwrap();
    captured.project().set_filesystem_fault(Some(
        yss_filesystem::FilesystemFaultPoint::FirstLiveReplacement,
    ));
    let failed = f.action(request.clone());
    captured.project().set_filesystem_fault(None);
    assert_eq!(
        failed.unwrap_err().code,
        CapabilityFailureCode::PersistenceUnavailable
    );
    let after = captured
        .project()
        .read_graph_editing(&project, &path)
        .unwrap();
    assert_eq!(after.state, before.state);
    assert_eq!(after.document, before.document);
    assert_eq!(std::fs::read(&file).unwrap(), saved);
    let AutomationCapabilityRequest::ApplyGraphEdit(edit) = &request else {
        panic!("edit request")
    };
    assert!(
        application
            .recover_automation_graph_edit(f.context.clone(), edit.clone())
            .unwrap()
            .is_none()
    );

    let result = f.action(request.clone()).unwrap();
    let committed = captured
        .project()
        .read_graph_editing(&project, &path)
        .unwrap();
    assert_eq!(committed.document.nodes.len(), 2);
    assert!(!committed.state.dirty && committed.state.can_undo);
    let persisted = std::fs::read(&file).unwrap();
    assert_ne!(persisted, saved);
    let retry = f.action(request).unwrap();
    assert_eq!(
        serde_json::to_value(retry).unwrap(),
        serde_json::to_value(result).unwrap()
    );
    assert_eq!(std::fs::read(file).unwrap(), persisted);
    assert_eq!(
        captured
            .project()
            .read_graph_editing(&project, &path)
            .unwrap()
            .state,
        committed.state
    );
}

#[test]
fn assistant_edits_current_graph_validates_runs_and_reads_actual_series_results() {
    let mut f = Fixture::new();
    let saved_path = f.directory.join("project").join(&f.path);
    let initial = f.inspect();
    let created = f.edit(vec![
        GraphEditOperation::CreateNode {
            client_id: Some("source".into()),
            node_type_id: "yssbi.dataframe.source.get".into(),
            resource_path: Some(format!("databases/{}", f.dataset)),
            x: 0.,
            y: 0.,
            user_label: None,
        },
        node("yssbi.dataframe.decompose", "columns"),
        node("yssbi.numeric.multiply", "product"),
        node("yssbi.debug.view", "view"),
        GraphEditOperation::CreateConstant {
            name: "scale".into(),
            value: GraphConstantLiteral::Integer(3),
            x: 0.,
            y: 200.,
            client_id: Some("scale".into()),
        },
        connect(port("$source", "dataframe"), port("$columns", "dataframe")),
        connect(port("$scale", "value"), port("$product", "right")),
        connect(port("$product", "result"), port("$view", "data")),
    ]);
    assert_eq!(created.created_nodes.len(), 5);
    assert_eq!(created.from_revision, initial.revision);
    assert_eq!(
        created.changes.base_semantic_input_hash,
        initial.semantic_input_hash
    );
    assert_eq!(created.changes.nodes.len(), 5);
    assert_eq!(created.changes.constants.len(), 1);
    assert!(!created.changes.ready);
    assert!(
        created
            .changes
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.blocking)
    );
    let column = created
        .changes
        .nodes
        .iter()
        .find(|node| node.node_id == created.created_nodes["columns"])
        .unwrap()
        .ports
        .iter()
        .find(|port| port.direction == "output" && port.schema.contains_key("x"))
        .unwrap();
    assert_eq!(column.maximum_connections, None);
    let x = column.address.clone();
    let AutomationCapabilityResult::GraphValidation(blocked) = f
        .action(AutomationCapabilityRequest::ValidateGraph(
            ValidateGraphRequest {
                graph_path: f.path.clone(),
                graph_hash: created.graph_hash.clone(),
            },
        ))
        .unwrap()
    else {
        panic!("validation")
    };
    assert!(!blocked.ready);
    assert!(
        blocked
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.blocking)
    );
    let AutomationCapabilityResult::GraphEditReceipt(connected) = f
        .action(AutomationCapabilityRequest::ApplyGraphEdit(
            ApplyGraphEditRequest {
                graph_path: created.graph_path.clone(),
                base_revision: created.to_revision,
                graph_hash: created.graph_hash.clone(),
                client_key: "connect-returned-column".into(),
                locale: "en-US".into(),
                operations: vec![connect(
                    x.clone(),
                    port(&created.created_nodes["product"], "left"),
                )],
            },
        ))
        .unwrap()
    else {
        panic!("edit receipt");
    };
    assert!(connected.changes.ready);
    assert_eq!(connected.changes.connections.len(), 1);
    assert!(
        connected
            .changes
            .nodes
            .iter()
            .any(|node| node.node_id == created.created_nodes["product"])
    );
    let inspected = f.inspect();
    assert_eq!(connected.to_revision, inspected.revision);
    assert_eq!(
        connected.changes.semantic_input_hash,
        inspected.semantic_input_hash
    );
    assert_eq!(connected.changes.diagnostics, inspected.diagnostics);
    for node in &connected.changes.nodes {
        assert_eq!(
            Some(node),
            inspected
                .nodes
                .iter()
                .find(|current| current.node_id == node.node_id)
        );
    }
    let saved_document = std::fs::read_to_string(&saved_path).unwrap();
    assert!(saved_document.contains(&created.created_nodes["product"]));
    let AutomationCapabilityResult::GraphValidation(validated) = f
        .action(AutomationCapabilityRequest::ValidateGraph(
            ValidateGraphRequest {
                graph_path: f.path.clone(),
                graph_hash: graph_hash(&f.document).unwrap(),
            },
        ))
        .unwrap()
    else {
        panic!("validation")
    };
    assert!(validated.ready, "{:?}", validated.diagnostics);
    let AutomationCapabilityResult::GraphExecution(run) = f
        .action(AutomationCapabilityRequest::ExecuteGraph(
            ExecuteGraphRequest {
                graph_path: f.path.clone(),
                graph_hash: validated.graph_hash,
            },
        ))
        .unwrap()
    else {
        panic!("run")
    };
    assert_eq!(run.status, "succeeded", "{:?}", run.failure_code);
    assert!(!run.results.is_empty());
    let product = run
        .results
        .iter()
        .find(|result| result.output.starts_with(&created.created_nodes["product"]))
        .unwrap();
    let AutomationCapabilityResult::ResultInspection(result) = f
        .application
        .as_ref()
        .unwrap()
        .invoke_automation_capability(
            f.context.clone(),
            AutomationCapabilityRequest::InspectResult(InspectResultRequest {
                execution_session_id: product.execution_session_id.clone(),
                part: None,
                result_id: product.result_id,
                offset: 0,
                limit: 20,
            }),
            &CapabilityControl::new(CancellationToken::default(), Duration::from_secs(5)),
        )
        .unwrap()
    else {
        panic!("result")
    };
    let ResultValueInspection::Table { rows, has_more, .. } = result.value else {
        panic!("table preview")
    };
    assert!(!has_more);
    let actual = rows
        .into_iter()
        .map(|row| match row {
            serde_json::Value::Array(items) => items.into_iter().next().unwrap(),
            _ => panic!("row"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        [
            serde_json::json!(3),
            serde_json::json!(6),
            serde_json::json!(9)
        ]
    );
    assert_eq!(
        std::fs::read_to_string(&saved_path).unwrap(),
        saved_document,
        "validation and execution must not write the file after the assistant edit was saved"
    );
    let hash = graph_hash(&f.document).unwrap();
    let before_save_revision = f.revision;
    let AutomationCapabilityResult::GraphSaved(saved) = f
        .action(AutomationCapabilityRequest::SaveGraph(SaveGraphRequest {
            graph_path: f.path.clone(),
            graph_hash: hash.clone(),
        }))
        .unwrap()
    else {
        panic!("save")
    };
    assert_eq!(saved.graph_hash, hash);
    assert_eq!(saved.from_revision, before_save_revision);
    assert_eq!(saved.resource_revision, f.revision);
    assert!(!saved.dirty && !saved.can_undo && !saved.can_redo);
    let serialized = std::fs::read_to_string(f.directory.join("project").join(&f.path)).unwrap();
    assert!(serialized.contains(&created.created_nodes["product"]));
    let fit = f.edit(vec![
        node("yssbi.statistics.linear.fit", "fit"),
        GraphEditOperation::AddPortInstance {
            node_id: "$fit".into(),
            template_key: "predictors".into(),
            client_id: Some("first".into()),
        },
        GraphEditOperation::AddPortInstance {
            node_id: "$fit".into(),
            template_key: "predictors".into(),
            client_id: Some("second".into()),
        },
        connect(x.clone(), port("$fit", "response")),
        connect(
            x,
            GraphEditPortRef::Instance {
                node_id: "$fit".into(),
                template_key: "predictors".into(),
                instance_id: "$first".into(),
            },
        ),
    ]);
    assert_eq!(fit.created_ports.len(), 2);
    let removed_port = f.edit(vec![
        GraphEditOperation::DisconnectPort {
            address: fit.created_ports["first"].clone(),
        },
        GraphEditOperation::RemovePortInstance {
            address: fit.created_ports["first"].clone(),
        },
    ]);
    let fit_node = removed_port
        .changes
        .nodes
        .iter()
        .find(|node| node.node_id == fit.created_nodes["fit"])
        .unwrap();
    assert!(
        !fit_node
            .ports
            .iter()
            .any(|port| port.address == fit.created_ports["first"])
    );
    assert!(
        fit_node
            .ports
            .iter()
            .any(|port| port.address == fit.created_ports["second"])
    );
    assert_eq!(removed_port.changes.removed_connection_ids.len(), 1);
    let fit_id = parse_node_id(&fit.created_nodes["fit"]).unwrap();
    let deleted = f.edit(vec![GraphEditOperation::DeleteNodes {
        node_ids: vec![fit_id.to_string()],
    }]);
    assert_eq!(deleted.changes.removed_node_ids, [fit_id.to_string()]);
    assert_eq!(deleted.changes.removed_connection_ids.len(), 1);
    assert!(f.document.connections.values().all(
        |connection| connection.input.node_id != fit_id && connection.output.node_id != fit_id
    ));
}

#[test]
fn graph_edit_receipts_include_downstream_columns_and_only_changed_entities() {
    let mut f = Fixture::new();
    f.inspect();
    let created = f.edit(vec![
        GraphEditOperation::CreateNode {
            client_id: Some("source".into()),
            node_type_id: "yssbi.dataframe.source.get".into(),
            resource_path: Some(format!("databases/{}", f.dataset)),
            x: 0.,
            y: 0.,
            user_label: None,
        },
        node("yssbi.dataframe.decompose", "columns"),
        node("yssbi.numeric.multiply", "unrelated"),
    ]);
    let initial_columns = created
        .changes
        .nodes
        .iter()
        .find(|node| node.node_id == created.created_nodes["columns"])
        .unwrap();
    assert!(
        !initial_columns
            .ports
            .iter()
            .any(|port| port.schema.contains_key("x"))
    );
    let connected = f.edit(vec![connect(
        port(&created.created_nodes["source"], "dataframe"),
        port(&created.created_nodes["columns"], "dataframe"),
    )]);
    let columns = connected
        .changes
        .nodes
        .iter()
        .find(|node| node.node_id == created.created_nodes["columns"])
        .unwrap();
    for name in ["x", "label"] {
        assert!(
            columns
                .ports
                .iter()
                .any(|port| port.direction == "output" && port.schema.contains_key(name))
        );
    }
    assert!(
        !connected
            .changes
            .nodes
            .iter()
            .any(|node| node.node_id == created.created_nodes["unrelated"])
    );
    assert_eq!(connected.changes.connections.len(), 1);
    assert_eq!(
        connected.changes.base_semantic_input_hash,
        created.changes.semantic_input_hash
    );
    assert_ne!(
        connected.changes.semantic_input_hash,
        created.changes.semantic_input_hash
    );
    let moved = f.edit(vec![GraphEditOperation::MoveNodes {
        positions: vec![GraphEditPosition {
            node_id: columns.node_id.clone(),
            x: 400.,
            y: 500.,
        }],
    }]);
    assert_eq!(moved.changes.nodes.len(), 1);
    assert_eq!(
        (moved.changes.nodes[0].x, moved.changes.nodes[0].y),
        (400., 500.)
    );
    assert_eq!(
        moved.changes.semantic_input_hash,
        connected.changes.semantic_input_hash
    );
    let duplicated = f.edit(vec![GraphEditOperation::DuplicateNodes {
        node_ids: vec![
            created.created_nodes["source"].clone(),
            created.created_nodes["columns"].clone(),
        ],
        offset_x: 50.,
        offset_y: 50.,
    }]);
    assert_eq!(
        duplicated
            .changes
            .nodes
            .iter()
            .filter(|node| !created.created_nodes.values().any(|id| id == &node.node_id))
            .count(),
        2
    );
    let connection_id = connected.changes.connections[0].connection_id.clone();
    let disconnected = f.edit(vec![GraphEditOperation::DisconnectConnections {
        connection_ids: vec![connection_id.clone()],
    }]);
    assert_eq!(disconnected.changes.removed_connection_ids, [connection_id]);
    let columns = disconnected
        .changes
        .nodes
        .iter()
        .find(|node| node.node_id == created.created_nodes["columns"])
        .unwrap();
    assert!(
        !columns
            .ports
            .iter()
            .any(|port| port.schema.contains_key("x"))
    );
    let wire = serde_json::to_value(AutomationCapabilityResult::GraphEditReceipt(
        connected.clone(),
    ))
    .unwrap();
    let decoded: AutomationCapabilityResult = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(
        decoded,
        AutomationCapabilityResult::GraphEditReceipt(connected)
    );
    assert_eq!(
        wire["payload"]["changes"]["nodes"][0]["ports"][0]["address"]["nodeId"],
        wire["payload"]["changes"]["nodes"][0]["nodeId"]
    );
}

#[test]
fn graph_edit_receipts_detect_changes_to_omitted_constant_values() {
    let mut f = Fixture::new();
    f.inspect();
    let created = f.edit(vec![GraphEditOperation::CreateConstant {
        name: "long-text".into(),
        value: GraphConstantLiteral::String("a".repeat(5_000)),
        x: 0.,
        y: 0.,
        client_id: Some("constant".into()),
    }]);
    let (id, mut constant) = f
        .document
        .constants
        .iter()
        .next()
        .map(|(id, constant)| (*id, constant.clone()))
        .unwrap();
    assert_eq!(
        created.changes.constants[&id.to_string()]["valueIncluded"],
        false
    );
    constant.data_value = yss_data_contract::DataValue::String("b".repeat(5_000).into());
    let changed = f.edit(vec![GraphEditOperation::SetConstant {
        id: id.to_string(),
        constant: Some(serde_json::to_value(constant).unwrap()),
    }]);
    let facts = changed
        .changes
        .constants
        .get(&id.to_string())
        .expect("changing an omitted value must still report the changed constant");
    assert_eq!(facts["valueIncluded"], false);
    assert!(facts.get("dataValue").is_none());
    assert_ne!(
        facts["contentHash"],
        created.changes.constants[&id.to_string()]["contentHash"]
    );
    assert_eq!(facts, &f.inspect().constants[&id.to_string()]);
}

#[test]
fn execute_graph_returns_its_committed_results_after_a_later_graph_edit() {
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    let mut f = Fixture::new();
    f.inspect();
    let created = f.edit(vec![
        node("yssbi.numeric.multiply", "product"),
        node("yssbi.debug.view", "view"),
        GraphEditOperation::SetLiteral {
            address: port("$product", "left"),
            literal: Some(serde_json::json!(2)),
        },
        GraphEditOperation::SetLiteral {
            address: port("$product", "right"),
            literal: Some(serde_json::json!(3)),
        },
        connect(port("$product", "result"), port("$view", "data")),
    ]);
    let application = f.application.as_ref().unwrap().clone();
    let captured = application.capture_session().unwrap();
    let project = captured.project_instance_id().clone();
    let path = GraphResourcePath::new(&f.path).unwrap();
    let product = parse_node_id(&created.created_nodes["product"]).unwrap();
    let edited = Arc::new(AtomicBool::new(false));
    let observed_result = Arc::new(AtomicU64::new(0));
    let _subscription = application
        .subscribe_graph_activity(&project, {
            let application = application.clone();
            let project = project.clone();
            let edited = Arc::clone(&edited);
            let observed_result = Arc::clone(&observed_result);
            Arc::new(move |activity| {
                let GraphActivity::Execution(event) = activity else {
                    return;
                };
                match event.kind() {
                    RunApplicationEventKind::ResultInspectionRequested { result_id, .. } => {
                        observed_result.store(result_id.get(), Ordering::SeqCst);
                    }
                    RunApplicationEventKind::RunCompleted
                        if !edited.swap(true, Ordering::SeqCst) =>
                    {
                        let version = captured
                            .project()
                            .read_graph_editing(&project, &path)
                            .unwrap()
                            .state
                            .version;
                        application
                            .edit_graph(
                                GraphEditRequest {
                                    project_instance_id: project.clone(),
                                    graph_path: path.clone(),
                                    version,
                                    operation_id: OperationId::new(),
                                    locale: "en-US".into(),
                                },
                                EditorGraphMutation::SetLiteral {
                                    address: PortAddress::declared(
                                        product,
                                        "left".parse().unwrap(),
                                    ),
                                    literal: Some(serde_json::json!(5)),
                                },
                            )
                            .unwrap();
                    }
                    _ => {}
                }
            })
        })
        .unwrap();
    let AutomationCapabilityResult::GraphExecution(run) = f
        .action(AutomationCapabilityRequest::ExecuteGraph(
            ExecuteGraphRequest {
                graph_path: f.path.clone(),
                graph_hash: created.graph_hash,
            },
        ))
        .unwrap()
    else {
        panic!("execution receipt");
    };
    assert!(edited.load(Ordering::SeqCst));
    assert_eq!(run.status, "succeeded");
    assert!(run.results_complete);
    assert_eq!(run.result_count, Some(run.results.len()));
    assert!(run.results.iter().any(|result| result.result_id
        == observed_result.load(Ordering::SeqCst)
        && Some(result.run_id) == run.run_id));
    assert_ne!(run.graph_hash, graph_hash(&f.document).unwrap());
}

#[test]
fn execute_graph_reports_when_its_result_references_are_bounded() {
    let mut f = Fixture::new();
    f.inspect();
    let limit = usize::from(CapabilityId::ExecuteGraph.descriptor().maximum_results);
    let producers = (0..=limit).collect::<Vec<_>>();
    for batch in producers.chunks(10) {
        let mut operations = Vec::new();
        for index in batch {
            let product = format!("product-{index}");
            let view = format!("view-{index}");
            let product_alias = format!("${product}");
            let view_alias = format!("${view}");
            operations.extend([
                node("yssbi.numeric.multiply", &product),
                node("yssbi.debug.view", &view),
                GraphEditOperation::SetLiteral {
                    address: port(&product_alias, "left"),
                    literal: Some(serde_json::json!(2)),
                },
                GraphEditOperation::SetLiteral {
                    address: port(&product_alias, "right"),
                    literal: Some(serde_json::json!(3)),
                },
                connect(port(&product_alias, "result"), port(&view_alias, "data")),
            ]);
        }
        f.edit(operations);
    }
    let AutomationCapabilityResult::GraphExecution(run) = f
        .action(AutomationCapabilityRequest::ExecuteGraph(
            ExecuteGraphRequest {
                graph_path: f.path.clone(),
                graph_hash: graph_hash(&f.document).unwrap(),
            },
        ))
        .unwrap()
    else {
        panic!("execution receipt");
    };
    assert_eq!(run.status, "succeeded", "{:?}", run.failure_code);
    assert_eq!(run.result_count, Some(limit + 1));
    assert_eq!(run.results.len(), limit);
    assert!(!run.results_complete);
    let encoded =
        serde_json::to_value(AutomationCapabilityResult::GraphExecution(run.clone())).unwrap();
    assert_eq!(encoded["payload"]["resultCount"], limit + 1);
    assert_eq!(encoded["payload"]["resultsComplete"], false);
    assert_eq!(
        serde_json::from_value::<AutomationCapabilityResult>(encoded).unwrap(),
        AutomationCapabilityResult::GraphExecution(run)
    );
}

#[test]
fn oversized_graph_edit_facts_are_rejected_before_committing() {
    let mut f = Fixture::new();
    let before = f.inspect();
    let saved_path = f.directory.join("project").join(&f.path);
    let before_file = std::fs::read(&saved_path).unwrap();
    let request = f.edit_request(
        (0..100)
            .map(|index| node("yssbi.numeric.multiply", &format!("node-{index}")))
            .collect(),
    );
    assert_eq!(
        f.action(request.clone()).unwrap_err().code,
        CapabilityFailureCode::ResultTooLarge
    );
    assert_eq!(f.inspect(), before);
    assert_eq!(std::fs::read(&saved_path).unwrap(), before_file);
    let AutomationCapabilityRequest::ApplyGraphEdit(request) = request else {
        unreachable!()
    };
    assert!(
        f.application
            .as_ref()
            .unwrap()
            .recover_automation_graph_edit(f.context.clone(), request)
            .unwrap()
            .is_none()
    );
}

#[test]
fn graph_edit_batches_preserve_parameters_reject_stale_versions_and_roll_back_failure() {
    let mut f = Fixture::new();
    let node_id = NodeId::new();
    let id = &node_id.to_string();
    f.document.nodes.insert(
        node_id,
        yss_graph_document::DocumentNode {
            id: node_id,
            node_type: "yssbi.dataframe.rename".parse().unwrap(),
            position: NodePosition { x: 0., y: 0. },
            user_label: None,
            parameters: [
                ("from".parse().unwrap(), serde_json::json!("x")),
                ("to".parse().unwrap(), serde_json::json!("first")),
            ]
            .into(),
        },
    );
    {
        let captured = f.application.as_ref().unwrap().capture_session().unwrap();
        let path = GraphResourcePath::new(&f.path).unwrap();
        let operation = captured
            .project()
            .capture_graph_overwrite_operation(
                captured.project_instance_id(),
                &path,
                OperationId::new(),
            )
            .unwrap();
        let receipt = captured
            .project()
            .commit_graph_candidate(operation.into_authority(), Arc::new(f.document.clone()))
            .unwrap();
        f.revision = receipt.to_revision.get();
    }
    let configured = f.edit(vec![GraphEditOperation::SetParameters {
        node_id: id.clone(),
        parameters: [("to".into(), serde_json::json!("second"))].into(),
    }]);
    let parameters = &configured.changes.nodes[0].parameters;
    assert_eq!(
        parameters
            .iter()
            .find(|parameter| parameter.key == "from")
            .unwrap()
            .value,
        Some(serde_json::json!("x"))
    );
    assert_eq!(
        parameters
            .iter()
            .find(|parameter| parameter.key == "to")
            .unwrap()
            .value,
        Some(serde_json::json!("second"))
    );
    assert_eq!(
        f.document.nodes[&node_id].parameters[&"from".parse().unwrap()],
        "x"
    );
    assert_eq!(
        f.document.nodes[&node_id].parameters[&"to".parse().unwrap()],
        "second"
    );
    let stale = f.edit_request(vec![GraphEditOperation::DeleteNodes {
        node_ids: vec![id.clone()],
    }]);
    let before = f.document.clone();
    let invalid = f.edit_request(vec![
        GraphEditOperation::MoveNodes {
            positions: vec![GraphEditPosition {
                node_id: id.clone(),
                x: 333.,
                y: 444.,
            }],
        },
        connect(port(id, "missing"), port(id, "source")),
    ]);
    let captured = f.application.as_ref().unwrap().capture_session().unwrap();
    let path = GraphResourcePath::new(&f.path).unwrap();
    let before_editing = captured
        .project()
        .read_graph_editing(captured.project_instance_id(), &path)
        .unwrap();
    assert!(f.action(invalid.clone()).is_err());
    assert_eq!(f.document, before);
    let after_editing = captured
        .project()
        .read_graph_editing(captured.project_instance_id(), &path)
        .unwrap();
    assert_eq!(before_editing.state, after_editing.state);
    assert_eq!(before_editing.document, after_editing.document);
    if let AutomationCapabilityRequest::ApplyGraphEdit(request) = invalid {
        assert!(
            f.application
                .as_ref()
                .unwrap()
                .recover_automation_graph_edit(f.context.clone(), request)
                .unwrap()
                .is_none()
        );
    }
    drop(captured);
    f.edit(vec![GraphEditOperation::MoveNodes {
        positions: vec![GraphEditPosition {
            node_id: id.clone(),
            x: 200.,
            y: 200.,
        }],
    }]);
    assert_eq!(
        f.action(stale).unwrap_err().code,
        CapabilityFailureCode::GraphDraftChanged
    );
    f.edit(vec![GraphEditOperation::DeleteNodes {
        node_ids: vec![id.clone()],
    }]);
    assert!(f.document.nodes.is_empty());
    for query in ["multiply 乘法", "decompose 列拆分", "ols 回归"] {
        let result = f
            .application
            .as_ref()
            .unwrap()
            .invoke_automation_capability(
                f.context.clone(),
                AutomationCapabilityRequest::SearchNodeCatalog(SearchNodeCatalogRequest {
                    query: query.into(),
                    locale: "zh-CN".into(),
                    limit: 20,
                }),
                &CapabilityControl::new(CancellationToken::default(), Duration::from_secs(5)),
            )
            .unwrap();
        assert!(
            matches!(result, AutomationCapabilityResult::NodeCatalogSearch(ref result) if !result.matches.is_empty()),
            "{query}"
        );
        if let AutomationCapabilityResult::NodeCatalogSearch(result) = result {
            assert!(
                result
                    .matches
                    .iter()
                    .all(|item| item.node_type_id != "yssbi.statistics.logit.fit")
            );
        }
    }
}

#[test]
fn gui_and_harness_retries_recover_original_commits_without_overwriting_later_edits() {
    let mut f = Fixture::new();
    f.inspect();
    let harness_request = f.edit_request(vec![node("yssbi.numeric.multiply", "original")]);
    let harness_result = f.action(harness_request.clone()).unwrap();
    let application = f.application.as_ref().unwrap().clone();
    let captured = application.capture_session().unwrap();
    let path = GraphResourcePath::new(&f.path).unwrap();
    let project = captured.project_instance_id().clone();
    let version = captured
        .project()
        .read_graph_editing(&project, &path)
        .unwrap()
        .state
        .version;
    let gui_request = GraphEditRequest {
        project_instance_id: project.clone(),
        graph_path: path.clone(),
        version,
        operation_id: OperationId::new(),
        locale: "en-US".into(),
    };
    let create = EditorGraphMutation::CreateNode {
        descriptor: yss_node_catalog::NodeCreation::Static {
            node_type_id: "yssbi.numeric.multiply".parse().unwrap(),
        },
        position: NodePosition { x: 0., y: 0. },
        user_label: None,
        connect_from: None,
    };
    let created = application
        .edit_graph(gui_request.clone(), create.clone())
        .unwrap();
    assert_eq!(created.update.document.nodes.len(), 2);
    let commit = application
        .graph_edit_receipt(&project, &path, version, gui_request.operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(commit.commit.editing, created.editing);
    let save_request = GraphEditRequest {
        version: created.editing.version,
        operation_id: OperationId::new(),
        ..gui_request.clone()
    };
    let saved = application
        .save_current_graph(save_request.clone())
        .unwrap();
    let id = *created.update.document.nodes.keys().next().unwrap();
    let move_request = GraphEditRequest {
        version: saved.graph.editing.version,
        operation_id: OperationId::new(),
        ..gui_request.clone()
    };
    let moved = application
        .edit_graph(
            move_request,
            EditorGraphMutation::MoveNodes {
                positions: vec![NodePositionMutation {
                    node_id: id,
                    position: NodePosition { x: 444., y: 555. },
                }],
            },
        )
        .unwrap();
    let save_retry = application.save_current_graph(save_request).unwrap();
    assert_eq!(save_retry.resource_revision, saved.resource_revision);
    assert_eq!(save_retry.graph.editing, moved.editing);
    assert!(save_retry.graph.editing.dirty);
    let create_retry = application.edit_graph(gui_request.clone(), create).unwrap();
    assert_eq!(create_retry.editing, moved.editing);
    assert_eq!(create_retry.update.document, moved.update.document);
    assert!(
        application
            .edit_graph(
                gui_request.clone(),
                EditorGraphMutation::DeleteNodes { node_ids: vec![id] }
            )
            .is_err()
    );
    assert_eq!(f.action(harness_request.clone()).unwrap(), harness_result);
    let AutomationCapabilityRequest::ApplyGraphEdit(request) = harness_request else {
        unreachable!()
    };
    assert_eq!(
        application
            .recover_automation_graph_edit(f.context.clone(), request)
            .unwrap()
            .map(AutomationCapabilityResult::GraphEditReceipt),
        Some(harness_result)
    );
    let undo_request = GraphEditRequest {
        version: moved.editing.version,
        operation_id: OperationId::new(),
        ..gui_request
    };
    let undo = application
        .change_graph_history(undo_request.clone(), false)
        .unwrap();
    let replay = application
        .change_graph_history(undo_request, false)
        .unwrap();
    assert_eq!(undo.editing, replay.editing);
    assert_eq!(undo.update.document, replay.update.document);
    assert!(!undo.editing.dirty && undo.editing.can_redo);
    let concurrent_edit = ApplyGraphEditRequest {
        graph_path: f.path.clone(),
        graph_hash: graph_hash(&undo.update.document).unwrap(),
        base_revision: undo.editing.version.revision.get(),
        client_key: "concurrent".into(),
        locale: "en-US".into(),
        operations: vec![GraphEditOperation::MoveNodes {
            positions: vec![GraphEditPosition {
                node_id: id.to_string(),
                x: 888.,
                y: 0.,
            }],
        }],
    };
    let gui = GraphEditRequest {
        project_instance_id: project.clone(),
        graph_path: path.clone(),
        version: undo.editing.version,
        operation_id: OperationId::new(),
        locale: "en-US".into(),
    };
    let barrier = std::sync::Barrier::new(2);
    let outcomes = std::thread::scope(|scope| {
        let desktop = scope.spawn(|| {
            barrier.wait();
            application
                .edit_graph(
                    gui,
                    EditorGraphMutation::MoveNodes {
                        positions: vec![NodePositionMutation {
                            node_id: id,
                            position: NodePosition { x: 777., y: 0. },
                        }],
                    },
                )
                .is_ok()
        });
        let assistant = scope.spawn(|| {
            barrier.wait();
            application
                .invoke_automation_capability(
                    f.context.clone(),
                    AutomationCapabilityRequest::ApplyGraphEdit(concurrent_edit),
                    &CapabilityControl::new(CancellationToken::default(), Duration::from_secs(15)),
                )
                .is_ok()
        });
        (desktop.join().unwrap(), assistant.join().unwrap())
    });
    assert_ne!(
        outcomes.0, outcomes.1,
        "exactly one writer may commit from the shared base version"
    );
    let current = captured
        .project()
        .read_graph_editing(&project, &path)
        .unwrap();
    assert_eq!(
        current.state.version.revision.get(),
        undo.editing.version.revision.get() + 1
    );
    assert_eq!(
        current.document.nodes[&id].position.x,
        if outcomes.0 { 777. } else { 888. }
    );
}
