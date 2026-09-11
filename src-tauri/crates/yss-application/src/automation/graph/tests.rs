use super::*;
use crate::execution::{ApplicationSessionEpoch, ApplicationSessionSlot};
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
        let backend = Arc::new(yss_sci_runtime::SciRuntimeBackend::new());
        let candidate = crate::execution::session_factory::build_current_project_candidate(
            ApplicationSessionEpoch::INITIAL,
            project,
            [],
            backend.clone(),
        )
        .unwrap();
        let application =
            ApplicationState::from_composition(Arc::new(ApplicationSessionSlot::new()), backend);
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
        let action = prepare_automation_graph_action(
            self.application.as_ref().unwrap(),
            self.context.clone(),
            request,
            AutomationGraphDraft {
                document: self.document.clone(),
                generation: self.revision,
                locale: "en-US".into(),
            },
            &CapabilityControl::new(CancellationToken::default(), Duration::from_secs(15)),
            |_| true,
        )?;
        if let AutomationGraphUpdate::Draft(update) = action.update {
            self.document = update.document;
            self.revision += 1;
        }
        Ok(action.result)
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
fn assistant_edits_current_draft_compiles_runs_and_reads_actual_series_results() {
    let mut f = Fixture::new();
    let initial = f.inspect();
    assert_eq!(initial.source, "draft");
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
    let graph = f.inspect();
    let column = graph
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
    let AutomationCapabilityResult::GraphCompilation(blocked) = f
        .action(AutomationCapabilityRequest::CompileGraph(
            CompileGraphRequest {
                graph_path: f.path.clone(),
                graph_hash: graph.graph_hash,
            },
        ))
        .unwrap()
    else {
        panic!("compile")
    };
    assert!(!blocked.ready);
    assert!(
        blocked
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.blocking)
    );
    f.edit(vec![connect(
        x.clone(),
        port(&created.created_nodes["product"], "left"),
    )]);
    let AutomationCapabilityResult::GraphCompilation(compiled) = f
        .action(AutomationCapabilityRequest::CompileGraph(
            CompileGraphRequest {
                graph_path: f.path.clone(),
                graph_hash: graph_hash(&f.document).unwrap(),
            },
        ))
        .unwrap()
    else {
        panic!("compile")
    };
    assert!(compiled.ready, "{:?}", compiled.diagnostics);
    let AutomationCapabilityResult::GraphExecution(run) = f
        .action(AutomationCapabilityRequest::ExecuteGraph(
            ExecuteGraphRequest {
                graph_path: f.path.clone(),
                graph_hash: compiled.graph_hash,
                artifact_id: compiled.artifact_id.unwrap(),
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
            ResultValueInspection::List { items, .. } => items.into_iter().next().unwrap(),
            _ => panic!("row"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        [
            ResultValueInspection::Unsigned(3),
            ResultValueInspection::Unsigned(6),
            ResultValueInspection::Unsigned(9)
        ]
    );
    let session = f.application.as_ref().unwrap().capture_session().unwrap();
    assert!(
        session.project().get_data().unwrap().graphs[&GraphResourcePath::new(&f.path).unwrap()]
            .document
            .nodes
            .is_empty(),
        "editing, compilation and execution must not save the draft"
    );
    drop(session);
    let hash = graph_hash(&f.document).unwrap();
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
    let serialized = std::fs::read_to_string(f.directory.join("project").join(&f.path)).unwrap();
    assert!(serialized.contains(&created.created_nodes["product"]));
    let fit = f.edit(vec![
        node("yssbi.statistics.ols.summary", "fit"),
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
    f.edit(vec![
        GraphEditOperation::DisconnectPort {
            address: fit.created_ports["first"].clone(),
        },
        GraphEditOperation::RemovePortInstance {
            address: fit.created_ports["first"].clone(),
        },
    ]);
    let fit_id = parse_node_id(&fit.created_nodes["fit"]).unwrap();
    f.edit(vec![GraphEditOperation::DeleteNodes {
        node_ids: vec![fit_id.to_string()],
    }]);
    assert!(f.document.connections.values().all(
        |connection| connection.input.node_id != fit_id && connection.output.node_id != fit_id
    ));
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
    f.edit(vec![GraphEditOperation::SetParameters {
        node_id: id.clone(),
        parameters: [("to".into(), serde_json::json!("second"))].into(),
    }]);
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
    assert!(f.action(invalid).is_err());
    assert_eq!(f.document, before);
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
            matches!(result, AutomationCapabilityResult::NodeCatalogSearch(result) if !result.matches.is_empty()),
            "{query}"
        );
    }
}
