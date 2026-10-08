use super::*;
use crate::session::{ApplicationSessionEpoch, ApplicationSessionSlot};
use std::time::Duration;

mod authoring;
mod inspection;

struct Fixture {
    directory: std::path::PathBuf,
    application: Option<ApplicationState>,
    context: CapabilityInvocationContext,
    path: String,
    document: GraphDocument,
    revision: u64,
    dataset: String,
}

#[test]
fn independent_file_factories_publish_distinct_event_and_function_documents() {
    let fixture = Fixture::new();
    let application = fixture.application.as_ref().unwrap();
    let session = application.capture_session().unwrap();
    application
        .create_function_graph(
            session.project_instance_id().clone(),
            "Compute".into(),
            OperationId::new(),
        )
        .unwrap();
    let index = session
        .project()
        .read_project_index(session.project_instance_id())
        .unwrap();
    assert_eq!(index.event_graphs.len(), 1);
    assert_eq!(index.function_graphs.len(), 1);
    let data = session.project().get_data().unwrap();
    let event = &data.graphs[&GraphResourcePath::new(&index.event_graphs[0].path).unwrap()];
    assert!(event.document.nodes.is_empty());
    assert!(event.function.is_none());
    let function = &data.graphs[&GraphResourcePath::new(&index.function_graphs[0].path).unwrap()];
    assert!(function.function.is_some());
    let mut node_types = function
        .document
        .nodes
        .values()
        .map(|node| node.node_type.as_str())
        .collect::<Vec<_>>();
    node_types.sort();
    assert_eq!(
        node_types,
        [
            "yssbi.project.function.entry",
            "yssbi.project.function.return"
        ]
    );
    for node in function.document.nodes.values() {
        assert!(
            node.parameters
                .values()
                .any(|value| value.as_str() == Some(index.function_graphs[0].path.as_str()))
        );
    }
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
            .create_event_graph(instance.clone(), "Main".into(), OperationId::new())
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
                None,
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
                &mut |_| {},
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
                    view: GraphInspectionView::Full,
                    ..InspectGraphRequest::overview(self.path.clone())
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
        port_counts: Default::default(),
        parameters: Default::default(),
        client_id: Some(alias.into()),
        node_type_id: kind.into(),
        resource_path: None,
        x: 100.,
        y: 100.,
        user_label: None,
    }
}
fn constant_node(
    alias: &str,
    name: &str,
    value: ConstantValueInput,
    position: model::NodePositionInput,
) -> GraphEditOperation {
    GraphEditOperation::CreateConstant {
        declaration: ConstantDeclaration {
            client_id: alias.into(),
            name: name.into(),
            value,
            description: String::new(),
            tags: vec![],
            reference_node: Some(ConstantReferenceNode {
                client_id: Some(alias.into()),
                position,
                label: None,
            }),
        },
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

    for run_id in [None, Some(u64::MAX)] {
        let AutomationCapabilityResult::GraphResults(results) = f
            .application
            .as_ref()
            .unwrap()
            .invoke_automation_capability(
                f.context.clone(),
                AutomationCapabilityRequest::ListGraphResults(ListGraphResultsRequest {
                    graph: GraphResourceRef::for_path(&f.path),
                    run_id,
                    node_ids: vec![],
                    outputs: vec![],
                    offset: 0,
                    limit: 50,
                }),
                &CapabilityControl::new(CancellationToken::default(), Duration::from_secs(15)),
                &mut |_| {},
            )
            .unwrap()
        else {
            panic!("graph results")
        };
        assert!(results.results.is_empty());
        assert_eq!(results.page.total, Some(0));
        assert_eq!(results.run_status.as_deref(), run_id.map(|_| "unavailable"));
    }
    assert!(!captured.project().has_resident_graph(&path).unwrap());
    assert_eq!(std::fs::read(&saved_path).unwrap(), saved);
    assert!(captured.execution_snapshot().is_empty());
    let missing = super::super::results::list_results(
        &captured,
        ListGraphResultsRequest {
            graph: GraphResourceRef::for_path("events/Missing.yssbi-event"),
            run_id: None,
            node_ids: vec![],
            outputs: vec![],
            offset: 0,
            limit: 50,
        },
    )
    .unwrap_err();
    assert_eq!(missing.code, CapabilityFailureCode::GraphUnavailable);

    let AutomationCapabilityResult::ProjectInspection(inspection) = f
        .application
        .as_ref()
        .unwrap()
        .invoke_automation_capability(
            f.context.clone(),
            AutomationCapabilityRequest::ListResources(ListResourcesRequest::default()),
            &CapabilityControl::new(CancellationToken::default(), Duration::from_secs(15)),
            &mut |_| {},
        )
        .unwrap()
    else {
        panic!("project inspection");
    };
    assert!(inspection.resources.iter().any(|resource| {
        resource.resource.kind == ProjectResourceKind::EventGraph && resource.resource.id == f.path
    }));
    assert!(inspection.resources.iter().any(|resource| {
        resource.resource.kind == ProjectResourceKind::Database && resource.resource.id == f.dataset
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
                port_counts: Default::default(),
                parameters: Default::default(),
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
            port_counts: Default::default(),
            parameters: Default::default(),
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
        constant_node(
            "scale",
            "scale",
            ConstantValueInput {
                data_type: yss_data_contract::ValueType::Scalar(
                    yss_data_contract::SemanticType::Numeric,
                ),
                data_value: yss_data_contract::DataValue::Integer(3),
                tabular: None,
            },
            model::NodePositionInput { x: 0., y: 200. },
        ),
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
                graph: GraphResourceRef::for_path(f.path.clone()),
                node_ids: vec![],
                offset: 0,
                limit: 100,
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
                graph: GraphResourceRef::for_path(f.path.clone()),
                node_ids: vec![],
                offset: 0,
                limit: 100,
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
                demand: yss_harness_contract::GraphExecutionDemand::Default,
                graph: GraphResourceRef::for_path(f.path.clone()),
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
    let AutomationCapabilityResult::ResultTablePage(result) = f
        .application
        .as_ref()
        .unwrap()
        .invoke_automation_capability(
            f.context.clone(),
            AutomationCapabilityRequest::ReadResultTable(ReadResultTableRequest {
                table_ref: TableRef::new(product.result_ref.clone(), None),
                columns: vec![],
                column_offset: 0,
                column_limit: 50,
                offset: 0,
                limit: 20,
            }),
            &CapabilityControl::new(CancellationToken::default(), Duration::from_secs(5)),
            &mut |_| {},
        )
        .unwrap()
    else {
        panic!("table");
    };
    let rows = result.rows;
    let has_more = result.page.has_more;
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

    let AutomationCapabilityResult::GraphExecution(source_run) = f
        .action(AutomationCapabilityRequest::ExecuteGraph(
            ExecuteGraphRequest {
                graph: GraphResourceRef::for_path(f.path.clone()),
                graph_hash: graph_hash(&f.document).unwrap(),
                demand: GraphExecutionDemand::Node {
                    node_id: created.created_nodes["source"].clone(),
                    mode: NodeExecutionMode::Dependencies,
                },
            },
        ))
        .unwrap()
    else {
        panic!("source run");
    };
    assert_eq!(source_run.status, "succeeded");
    let timing = source_run.timing.as_ref().expect("admitted run timing");
    assert!(timing.elapsed_ms >= timing.running_ms);
    let public_run = yss_harness_contract::model::capability_result(
        &AutomationCapabilityResult::GraphExecution(source_run.clone()),
    )
    .unwrap();
    assert_eq!(
        public_run["payload"]["timing"],
        serde_json::to_value(timing).unwrap()
    );
    let query = ListGraphResultsRequest {
        graph: GraphResourceRef::for_path(f.path.clone()),
        run_id: source_run.run_id,
        node_ids: vec![created.created_nodes["source"].clone()],
        outputs: vec![],
        offset: 0,
        limit: 1,
    };
    let AutomationCapabilityResult::GraphResults(list) = f
        .action(AutomationCapabilityRequest::ListGraphResults(query.clone()))
        .unwrap()
    else {
        panic!("results");
    };
    assert_eq!(list.run_status.as_deref(), Some("succeeded"));
    assert_eq!(list.run_timing, source_run.timing);
    assert_eq!(list.page.total, Some(1));
    assert_eq!(list.results[0].validity, ResultValidity::CurrentValid);
    let table_result = list.results[0].result_ref.clone();
    let session = f.application.as_ref().unwrap().capture_session().unwrap();
    session.presentation.attach_workbench();
    let opened = f
        .action(AutomationCapabilityRequest::RequestUiIntent(
            RequestUiIntent {
                client_key: "open-produced-result".into(),
                input: model::RequestUiIntentInput {
                    intent: model::UiIntentInput::OpenResult {
                        result_ref: table_result.clone(),
                    },
                },
            },
        ))
        .unwrap();
    let visible = model::capability_result(&opened).unwrap();
    assert_eq!(
        visible["payload"]["intent"]["resultRef"],
        serde_json::json!(table_result)
    );
    let inspected = f
        .action(AutomationCapabilityRequest::InspectUiIntent(
            yss_ui_contract::InspectUiIntentRequest {
                id: visible["payload"]["id"].as_str().unwrap().into(),
            },
        ))
        .unwrap();
    assert_eq!(
        model::capability_result(&inspected).unwrap()["payload"],
        visible["payload"]
    );
    session.presentation.detach_workbench();
    let AutomationCapabilityResult::ResultInspection(overview) = f
        .action(AutomationCapabilityRequest::InspectResult(
            InspectResultRequest {
                result_ref: table_result.clone(),
                schema_offset: 1,
                schema_limit: 1,
            },
        ))
        .unwrap()
    else {
        panic!("schema");
    };
    let ResultValueInspection::Tabular {
        table_ref,
        columns,
        schema_page,
        ..
    } = overview.value
    else {
        panic!("tabular");
    };
    assert_eq!(columns[0].name, "label");
    assert_eq!(schema_page.total, Some(2));
    let table_request = ReadResultTableRequest {
        table_ref,
        columns: vec!["label".into()],
        column_offset: 0,
        column_limit: 50,
        offset: 1,
        limit: 1,
    };
    let AutomationCapabilityResult::ResultTablePage(page) = f
        .action(AutomationCapabilityRequest::ReadResultTable(
            table_request.clone(),
        ))
        .unwrap()
    else {
        panic!("selected column");
    };
    assert_eq!(page.columns.len(), 1);
    assert_eq!(page.rows, [serde_json::json!(["b"])]);
    assert_eq!(page.page.next_offset, Some(2));
    let mut invalid_column = table_request;
    invalid_column.columns = vec!["missing".into()];
    assert_eq!(
        f.action(AutomationCapabilityRequest::ReadResultTable(invalid_column))
            .unwrap_err()
            .code,
        CapabilityFailureCode::InvalidRequest
    );
    let mut next = query;
    next.offset = 1;
    let AutomationCapabilityResult::GraphResults(end) = f
        .action(AutomationCapabilityRequest::ListGraphResults(next))
        .unwrap()
    else {
        panic!("last page");
    };
    assert!(end.results.is_empty() && !end.page.has_more);
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
    let before_form = f.inspect();
    let AutomationCapabilityResult::NodeTypeInspection(definitions) = f
        .action(AutomationCapabilityRequest::InspectNodeType(
            InspectNodeTypeRequest {
                type_ids: vec!["yssbi.statistics.linear.fit".into()],
                locale: "en-US".into(),
            },
        ))
        .unwrap()
    else {
        panic!("node definition");
    };
    let definition = &definitions.types[0];
    let configuration = &definition.configuration_schema;
    let configuration_validator = jsonschema::validator_for(configuration).unwrap();
    assert!(configuration_validator.is_valid(&serde_json::json!({
        "parameters": {"method": "OLS"}, "portCounts": {"x": 4}
    })));
    assert!(!configuration_validator.is_valid(&serde_json::json!({
        "parameters": {"method": "not_a_method"}, "portCounts": {"x": 4}
    })));
    assert!(!configuration_validator.is_valid(&serde_json::json!({
        "parameters": {"method": "OLS"}, "portCounts": {"y": 2}
    })));
    assert!(
        definition.ports.iter().any(|port| port.key == "x"
            && matches!(port.count, NodePortCountPolicy::Configurable { .. }))
    );
    let application = f.application.as_ref().unwrap();
    let session = application.capture_session().unwrap();
    let form = application
        .node_creation_form(
            session.project_instance_id(),
            &definition.type_id.parse().unwrap(),
            [("method".parse().unwrap(), serde_json::json!("OLS"))].into(),
            [("x".parse().unwrap(), 4)].into(),
            "en-US",
        )
        .unwrap();
    assert_eq!(f.inspect(), before_form);
    let mut configured = node(&definition.type_id, "configured");
    if let GraphEditOperation::CreateNode {
        parameters,
        port_counts,
        ..
    } = &mut configured
    {
        *parameters = form
            .values
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect();
        *port_counts = form
            .port_counts
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect();
    }
    let mut operations = vec![configured, connect(x.clone(), port("$configured", "y"))];
    operations.extend((0..4).map(|index| {
        connect(
            x.clone(),
            GraphEditPortRef::Instance {
                node_id: "$configured".into(),
                template_key: "x".into(),
                instance_id: format!("$configured.x[{index}]"),
            },
        )
    }));
    let configured = f.edit(operations);
    assert_eq!(configured.created_ports.len(), 4);
    let configured_id = parse_node_id(&configured.created_nodes["configured"]).unwrap();
    assert_eq!(
        f.document
            .port_bindings
            .keys()
            .filter(|address| address.node_id == configured_id)
            .count(),
        4
    );
    assert_eq!(
        f.document
            .connections
            .values()
            .filter(|connection| connection.input.node_id == configured_id)
            .count(),
        5
    );
    for index in 0..4 {
        let address =
            parse_edit_port(configured.created_ports[&format!("configured.x[{index}]")].clone())
                .unwrap();
        assert!(
            f.document
                .connections
                .values()
                .any(|connection| connection.input == address)
        );
    }
    let fit = f.edit(vec![
        node("yssbi.statistics.linear.fit", "fit"),
        GraphEditOperation::AddPortInstance {
            node_id: "$fit".into(),
            template_key: "x".into(),
            client_id: Some("first".into()),
        },
        GraphEditOperation::AddPortInstance {
            node_id: "$fit".into(),
            template_key: "x".into(),
            client_id: Some("second".into()),
        },
        connect(x.clone(), port("$fit", "y")),
        connect(
            x,
            GraphEditPortRef::Instance {
                node_id: "$fit".into(),
                template_key: "x".into(),
                instance_id: "$first".into(),
            },
        ),
    ]);
    assert_eq!(fit.created_ports.len(), 3);
    assert!(fit.created_ports.contains_key("fit.x[0]"));
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
fn agent_graph_execution_inherits_dependency_reads_and_preserves_explicit_version_guards() {
    let mut fixture = Fixture::new();
    fixture.inspect();
    let created = fixture.edit(vec![
        GraphEditOperation::CreateNode {
            port_counts: Default::default(),
            parameters: Default::default(),
            client_id: Some("source".into()),
            node_type_id: "yssbi.dataframe.source.get".into(),
            resource_path: Some(format!("databases/{}", fixture.dataset)),
            x: 0.,
            y: 0.,
            user_label: None,
        },
        node("yssbi.debug.view", "view"),
        connect(port("$source", "dataframe"), port("$view", "data")),
    ]);
    assert!(created.changes.ready);
    let graph = ProjectResourceRef {
        kind: ProjectResourceKind::EventGraph,
        id: fixture.path.clone(),
    };
    let database = ProjectResourceRef {
        kind: ProjectResourceKind::Database,
        id: fixture.dataset.clone(),
    };
    let mut grants = Vec::new();
    for resource in [graph, database] {
        let AutomationCapabilityResult::ResourceInspection(value) = fixture
            .action(AutomationCapabilityRequest::InspectResource(
                InspectResourceRequest {
                    resource: resource.clone(),
                },
            ))
            .unwrap()
        else {
            panic!("resource")
        };
        let visible = model::capability_result(&AutomationCapabilityResult::ResourceInspection(
            value.clone(),
        ))
        .unwrap();
        assert_eq!(
            visible["payload"]["content"],
            serde_json::json!({"kind":"metadata"})
        );
        grants.push(AgentResourceAccess {
            resource,
            version: Some(value.version),
            operations: vec![
                AgentResourceOperation::Inspect,
                AgentResourceOperation::Execute,
            ],
        });
    }
    let context = fixture.context.clone();
    let run = AutomationCapabilityRequest::ExecuteGraph(ExecuteGraphRequest {
        demand: yss_harness_contract::GraphExecutionDemand::Default,
        graph: GraphResourceRef::for_path(fixture.path.clone()),
        graph_hash: created.graph_hash,
    });
    fixture.context = context.clone().with_agent(AgentInvocationScope {
        run_id: AgentRunId::try_new("restricted-stats").unwrap(),
        role: AgentRole::Stats,
        task: Some(AgentTaskScope {
            resources: vec![grants[0].clone()],
            ..Default::default()
        }),
    });
    let AutomationCapabilityResult::GraphExecution(inherited) =
        fixture.action(run.clone()).unwrap()
    else {
        panic!("execution")
    };
    assert_eq!(
        inherited.status, "succeeded",
        "{:?}",
        inherited.failure_code
    );
    assert!(inherited.run_id.is_some());
    let mut stale = grants.clone();
    stale[1].version.as_mut().unwrap().revision += 1;
    fixture.context = context.clone().with_agent(AgentInvocationScope {
        run_id: AgentRunId::try_new("stale-dataset").unwrap(),
        role: AgentRole::Stats,
        task: Some(AgentTaskScope {
            resources: stale,
            ..Default::default()
        }),
    });
    let AutomationCapabilityResult::GraphExecution(stale) = fixture.action(run.clone()).unwrap()
    else {
        panic!("execution")
    };
    assert_eq!(
        stale.failure_code.as_deref(),
        Some("resource_version_changed")
    );
    assert!(
        stale
            .failure_location
            .as_ref()
            .unwrap()
            .contains(&fixture.dataset)
    );
    assert!(
        stale
            .failure_location
            .as_ref()
            .unwrap()
            .contains("expected")
    );

    fixture.context = context.with_agent(AgentInvocationScope {
        run_id: AgentRunId::try_new("scoped-stats").unwrap(),
        role: AgentRole::Stats,
        task: Some(AgentTaskScope {
            resources: grants,
            ..Default::default()
        }),
    });
    let AutomationCapabilityResult::GraphExecution(allowed) = fixture.action(run).unwrap() else {
        panic!("execution")
    };
    assert_eq!(allowed.status, "succeeded", "{:?}", allowed.failure_code);
}

#[test]
fn selected_output_runs_its_dependencies_without_unrelated_readiness_or_resource_grants() {
    use crate::graph::run::{ExecutionApplicationError, ResourceBindingError, RunDemand};
    use yss_graph_execution::plan::{PlanGraphId, PlanOutputRef, PlanPortAddress};
    let mut fixture = Fixture::new();
    fixture.inspect();
    let created = fixture.edit(vec![
        constant_node(
            "scale",
            "scale",
            ConstantValueInput {
                data_type: yss_data_contract::ValueType::Scalar(
                    yss_data_contract::SemanticType::Numeric,
                ),
                data_value: yss_data_contract::DataValue::Integer(2),
                tabular: None,
            },
            model::NodePositionInput::default(),
        ),
        node("yssbi.numeric.multiply", "product"),
        connect(port("$scale", "value"), port("$product", "left")),
        GraphEditOperation::SetLiteral {
            address: port("$product", "right"),
            literal: Some(serde_json::json!(3)),
        },
        node("yssbi.numeric.divide", "unfinished"),
        node("yssbi.debug.view", "cached_view"),
        connect(port("$product", "result"), port("$cached_view", "data")),
        GraphEditOperation::CreateNode {
            port_counts: Default::default(),
            parameters: Default::default(),
            client_id: Some("table".into()),
            node_type_id: "yssbi.dataframe.source.get".into(),
            resource_path: Some(format!("databases/{}", fixture.dataset)),
            x: 0.0,
            y: 0.0,
            user_label: None,
        },
        node("yssbi.dataframe.project", "project"),
        GraphEditOperation::SetParameters {
            node_id: "$project".into(),
            parameters: BTreeMap::from([("columns".into(), serde_json::json!(["x"]))]),
        },
        connect(port("$table", "dataframe"), port("$project", "source")),
    ]);
    assert!(!created.changes.ready);
    let mut validate = |node_ids: Vec<String>, offset| {
        let AutomationCapabilityResult::GraphValidation(result) = fixture
            .action(AutomationCapabilityRequest::ValidateGraph(
                ValidateGraphRequest {
                    graph: GraphResourceRef::for_path(fixture.path.clone()),
                    graph_hash: created.graph_hash.clone(),
                    node_ids,
                    offset,
                    limit: 1,
                },
            ))
            .unwrap()
        else {
            panic!("validation")
        };
        result
    };
    let local = validate(vec![created.created_nodes["product"].clone()], 0);
    assert!(local.ready);
    assert_eq!(
        local.scope_node_count, 2,
        "validation includes the real dependency closure"
    );
    assert!(local.diagnostics.is_empty());
    let whole = validate(vec![], 0);
    assert!(!whole.ready);
    assert!(whole.page.total.unwrap() > 0);
    assert!(
        !validate(vec![], whole.page.total.unwrap()).ready,
        "paging cannot hide blocking diagnostics from readiness"
    );
    let application = fixture.application.as_ref().unwrap();
    let session = application.capture_session().unwrap();
    let graph = GraphResourcePath::new(&fixture.path).unwrap();
    let context =
        crate::graph::inputs::GraphResolutionContext::capture(&session, &fixture.document).unwrap();
    let analysis = context.resolve(
        &session,
        &graph,
        &Arc::new(fixture.document.clone()),
        "en-US",
    );
    let request = |demand| {
        RunGraphRequest::new(
            session.project_instance_id().clone(),
            graph.clone(),
            Arc::new(fixture.document.clone()),
            *analysis.semantic_input_hash(),
        )
        .with_demand(demand)
        .with_resource_authorizations(Vec::new())
    };
    let output = |alias: &str, key: &str| {
        PlanOutputRef::new(
            PlanGraphId::new(graph.as_str().into()).unwrap(),
            PlanPortAddress::new(
                PortAddress::declared(
                    parse_node_id(&created.created_nodes[alias]).unwrap(),
                    key.parse().unwrap(),
                )
                .to_string()
                .into(),
            )
            .unwrap(),
        )
    };
    let demand = |output| RunDemand::Outputs {
        outputs: Box::new([output]),
        include_default_results: false,
        reuse_inputs: false,
    };
    assert!(matches!(
        crate::graph::run::run_graph(application, request(RunDemand::Default)),
        Err(ExecutionApplicationError::GraphNotReady)
    ));
    let target = output("product", "result");
    let node_demand = |alias: &str, mode| RunDemand::Node {
        node_id: parse_node_id(&created.created_nodes[alias]).unwrap(),
        mode,
    };
    let missing = crate::graph::run::run_graph(
        application,
        request(node_demand(
            "product",
            yss_graph_execution::plan::NodeExecutionMode::CurrentInputs,
        )),
    );
    assert!(
        matches!(missing, Err(ExecutionApplicationError::PreparedExecution(error))
        if error.failure().code == yss_graph_execution::error::RunFailureCode::InputResultUnavailable)
    );
    crate::graph::run::run_graph(application, request(demand(target.clone()))).unwrap();
    let result = session.execution().query_pin_result(&target).unwrap();
    assert_eq!(
        result.value().value(),
        &yss_node_kernel::RuntimeValue::Scalar(yss_data_contract::TabularScalar::Integer(6))
    );
    let result_id = result.provenance().result_id();
    let mut observed = Vec::new();
    let receipt = run_graph_with_sink(
        application,
        request(node_demand(
            "cached_view",
            yss_graph_execution::plan::NodeExecutionMode::CurrentInputs,
        )),
        |event| {
            if let RunApplicationEventKind::ResultInspectionRequested { result_id, .. } =
                event.kind()
            {
                observed.push(*result_id);
            }
            true
        },
    )
    .unwrap();
    assert!(receipt.results.is_empty());
    assert_eq!(observed, [result_id]);
    assert!(
        session
            .execution()
            .query_pin_result(&output("scale", "value"))
            .is_some()
    );
    assert!(matches!(
        crate::graph::run::run_graph(application, request(demand(output("table", "dataframe")))),
        Err(ExecutionApplicationError::ResourceBindings(
            ResourceBindingError::ScopeDenied { .. }
        ))
    ));
    let AutomationCapabilityResult::GraphExecution(executed) = fixture
        .action(AutomationCapabilityRequest::ExecuteGraph(
            ExecuteGraphRequest {
                graph: GraphResourceRef::for_path(fixture.path.clone()),
                graph_hash: created.graph_hash.clone(),
                demand: yss_harness_contract::GraphExecutionDemand::Node {
                    node_id: created.created_nodes["product"].clone(),
                    mode: yss_harness_contract::NodeExecutionMode::CurrentInputs,
                },
            },
        ))
        .unwrap()
    else {
        panic!("execution result");
    };
    assert_eq!(executed.status, "succeeded", "{:?}", executed.failure_code);
    assert_eq!(executed.result_count, Some(1));
    let mut run_node = |alias: &str, mode| {
        let AutomationCapabilityResult::GraphExecution(result) = fixture
            .action(AutomationCapabilityRequest::ExecuteGraph(
                ExecuteGraphRequest {
                    graph: GraphResourceRef::for_path(fixture.path.clone()),
                    graph_hash: created.graph_hash.clone(),
                    demand: yss_harness_contract::GraphExecutionDemand::Node {
                        node_id: created.created_nodes[alias].clone(),
                        mode,
                    },
                },
            ))
            .unwrap()
        else {
            panic!("execution result");
        };
        result
    };
    use yss_harness_contract::NodeExecutionMode::{CurrentInputs, Dependencies};
    let projected = run_node("project", Dependencies);
    assert_eq!(
        projected.status, "succeeded",
        "{:?}",
        projected.failure_code
    );
    assert_eq!(projected.result_count, Some(1));
    assert!(
        session
            .execution()
            .query_pin_result(&output("table", "dataframe"))
            .is_none()
    );
    let missing = run_node("project", CurrentInputs);
    assert_eq!(
        missing.failure_code.as_deref(),
        Some("input_result_unavailable")
    );
    assert!(missing.failure_location.is_some());
    assert_eq!(run_node("table", CurrentInputs).status, "succeeded");
    assert_eq!(run_node("project", CurrentInputs).status, "succeeded");
}

#[test]
fn graph_edit_receipts_include_downstream_columns_and_only_changed_entities() {
    let mut f = Fixture::new();
    f.inspect();
    let created = f.edit(vec![
        GraphEditOperation::CreateNode {
            port_counts: Default::default(),
            parameters: Default::default(),
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
    let created = f.edit(vec![constant_node(
        "constant",
        "long-text",
        ConstantValueInput {
            data_type: yss_data_contract::ValueType::Scalar(yss_data_contract::SemanticType::Text),
            data_value: yss_data_contract::DataValue::String("a".repeat(5_000).into()),
            tabular: None,
        },
        model::NodePositionInput::default(),
    )]);
    let (id, constant) = f
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
    let changed = f.edit(vec![GraphEditOperation::UpdateConstant {
        update: ConstantUpdate {
            constant_id: id.to_string(),
            name: None,
            value: Some(ConstantValueInput {
                data_type: constant.data_type,
                data_value: yss_data_contract::DataValue::String("b".repeat(5_000).into()),
                tabular: None,
            }),
            description: None,
            tags: None,
        },
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
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};
    let mut f = Fixture::new();
    f.inspect();
    let created = f.edit(vec![
        node("yssbi.numeric.multiply", "product"),
        node("yssbi.debug.view", "view"),
        node("yssbi.debug.view", "view2"),
        GraphEditOperation::SetLiteral {
            address: port("$product", "left"),
            literal: Some(serde_json::json!(2)),
        },
        GraphEditOperation::SetLiteral {
            address: port("$product", "right"),
            literal: Some(serde_json::json!(3)),
        },
        connect(port("$product", "result"), port("$view", "data")),
        connect(port("$product", "result"), port("$view2", "data")),
    ]);
    let application = f.application.as_ref().unwrap().clone();
    let captured = application.capture_session().unwrap();
    let project = captured.project_instance_id().clone();
    let path = GraphResourcePath::new(&f.path).unwrap();
    let product = parse_node_id(&created.created_nodes["product"]).unwrap();
    let edited = Arc::new(AtomicBool::new(false));
    let observed_requests = Arc::new(Mutex::new(Vec::new()));
    let _subscription = application
        .subscribe_graph_activity(&project, {
            let application = application.clone();
            let project = project.clone();
            let edited = Arc::clone(&edited);
            let observed_requests = Arc::clone(&observed_requests);
            Arc::new(move |activity| {
                let GraphActivity::Execution(event) = activity else {
                    return;
                };
                match event.kind() {
                    RunApplicationEventKind::ResultInspectionRequested { result_id, source } => {
                        observed_requests
                            .lock()
                            .unwrap()
                            .push((result_id.get(), source.clone()));
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
                demand: yss_harness_contract::GraphExecutionDemand::Default,
                graph: GraphResourceRef::for_path(f.path.clone()),
                graph_hash: created.graph_hash,
            },
        ))
        .unwrap()
    else {
        panic!("execution receipt");
    };
    assert_eq!(run.status, "succeeded", "{:?}", run.failure_code);
    assert!(edited.load(Ordering::SeqCst));
    assert!(run.results_complete);
    assert_eq!(run.result_count, Some(run.results.len()));
    let requests = observed_requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].0, requests[1].0);
    for view in ["view", "view2"] {
        assert!(requests.iter().any(|(_, source)| {
            source
                .node()
                .is_some_and(|node| node.as_str() == created.created_nodes[view])
        }));
    }
    assert!(
        run.results
            .iter()
            .any(|result| result.result_ref.result_id() == requests[0].0
                && Some(result.run_id) == run.run_id)
    );
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
                demand: yss_harness_contract::GraphExecutionDemand::Default,
                graph: GraphResourceRef::for_path(f.path.clone()),
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
fn graph_edit_receipts_admit_large_batches_and_reject_oversized_facts_before_committing() {
    let mut f = Fixture::new();
    f.inspect();
    let batch = f.edit_request(
        (0..100)
            .map(|index| node("yssbi.numeric.multiply", &format!("node-{index}")))
            .collect(),
    );
    let committed = f.action(batch.clone()).unwrap();
    assert!(serde_json::to_vec(&committed).unwrap().len() > 64 * 1024);
    assert_eq!(f.document.nodes.len(), 100);
    assert_eq!(f.action(batch).unwrap(), committed);
    let before = f.inspect();
    let saved_path = f.directory.join("project").join(&f.path);
    let before_file = std::fs::read(&saved_path).unwrap();
    let request = f.edit_request(
        (0..100)
            .map(|index| {
                let mut operation = node("yssbi.statistics.linear.fit", &format!("fit-{index}"));
                let GraphEditOperation::CreateNode { port_counts, .. } = &mut operation else {
                    unreachable!()
                };
                port_counts.insert("x".into(), 32);
                operation
            })
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
    let inspected = f.inspect();
    let original_context = f.context.clone();
    f.context = original_context
        .clone()
        .with_graph_observation(Some("f".repeat(64)));
    let unchanged = f.document.clone();
    assert_eq!(
        f.action(f.edit_request(vec![node("yssbi.debug.view", "rejected")]))
            .unwrap_err()
            .code,
        CapabilityFailureCode::RevisionConflict
    );
    assert_eq!(
        f.document, unchanged,
        "a changed semantic basis must be rejected before commit"
    );
    f.context = original_context
        .clone()
        .with_graph_observation(Some(inspected.semantic_input_hash));
    let mut create = node("yssbi.dataframe.groupby", "group");
    let GraphEditOperation::CreateNode { parameters, .. } = &mut create else {
        unreachable!()
    };
    *parameters = [
        ("keys".into(), serde_json::json!(["label"])),
        ("mean".into(), serde_json::json!(["x"])),
    ]
    .into();
    let created = f.edit(vec![
        create.clone(),
        node("yssbi.debug.view", "view"),
        connect(port("$group", "result"), port("$view", "data")),
    ]);
    f.context = original_context;
    let node_id = parse_node_id(&created.created_nodes["group"]).unwrap();
    let id = &node_id.to_string();
    assert_eq!(f.document.connections.len(), 1);
    let before_invalid_create = f.document.clone();
    let GraphEditOperation::CreateNode { parameters, .. } = &mut create else {
        unreachable!()
    };
    parameters.insert("keys".into(), serde_json::json!([42]));
    let invalid_create = f.edit_request(vec![node("yssbi.debug.view", "discarded"), create]);
    assert!(f.action(invalid_create).is_err());
    assert_eq!(f.document, before_invalid_create);
    let configured = f.edit(vec![GraphEditOperation::SetParameters {
        node_id: id.clone(),
        parameters: [("mean".into(), serde_json::json!(["amount"]))].into(),
    }]);
    let parameters = &configured.changes.nodes[0].parameters;
    let mean = parameters
        .iter()
        .find(|parameter| parameter.key == "mean")
        .unwrap();
    assert!(
        mean.options.is_none(),
        "unconnected choices must not look like a known empty schema"
    );
    assert!(mean.context_hint.is_some());
    assert_eq!(
        parameters
            .iter()
            .find(|parameter| parameter.key == "keys")
            .unwrap()
            .value,
        Some(serde_json::json!(["label"]))
    );
    assert_eq!(
        parameters
            .iter()
            .find(|parameter| parameter.key == "mean")
            .unwrap()
            .value,
        Some(serde_json::json!(["amount"]))
    );
    assert_eq!(
        f.document.nodes[&node_id].parameters[&"keys".parse().unwrap()],
        serde_json::json!(["label"])
    );
    assert_eq!(
        f.document.nodes[&node_id].parameters[&"mean".parse().unwrap()],
        serde_json::json!(["amount"])
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
        node_ids: vec![id.clone(), created.created_nodes["view"].clone()],
    }]);
    assert!(f.document.nodes.is_empty());
    for query in ["multiply 乘法", "decompose 列拆分", "ols 回归"] {
        let result = f
            .application
            .as_ref()
            .unwrap()
            .invoke_automation_capability(
                f.context.clone(),
                AutomationCapabilityRequest::BrowseNodes(BrowseNodesRequest {
                    category: None,
                    offset: 0,
                    query: query.into(),
                    locale: "zh-CN".into(),
                    limit: 20,
                }),
                &CapabilityControl::new(CancellationToken::default(), Duration::from_secs(5)),
                &mut |_| {},
            )
            .unwrap();
        assert!(
            matches!(result, AutomationCapabilityResult::NodeCatalogPage(ref result) if !result.matches.is_empty()),
            "{query}"
        );
        if let AutomationCapabilityResult::NodeCatalogPage(result) = result {
            let application = f.application.as_ref().unwrap();
            let session = application.capture_session().unwrap();
            let (_, _, _, catalog) = application
                .localized_node_catalog(crate::graph::catalog::LocalizedCatalogRequest::new(
                    session.project_instance_id().clone(),
                    "zh-CN",
                ))
                .unwrap()
                .into_transport_parts()
                .into_fields();
            assert!(result.matches.iter().all(|item| {
                catalog.items.iter().any(|entry| {
                    entry.node_type_id.as_ref() == item.type_id.as_str() && entry.available
                })
            }));
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
        port_counts: Default::default(),
        parameters: Default::default(),
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
    let commit = captured
        .project()
        .graph_edit_command_receipt(
            &project,
            &path,
            version.session_id,
            gui_request.operation_id,
        )
        .unwrap()
        .unwrap();
    assert_eq!(commit.request_version, version);
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
                    &mut |_| {},
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
