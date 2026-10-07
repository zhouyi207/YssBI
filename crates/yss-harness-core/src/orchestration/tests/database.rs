use super::*;
use serde_json::json;

struct DatabaseGateway(Mutex<(u64, usize)>);
impl CapabilityGatewayPort for DatabaseGateway {
    fn invoke<'a>(
        &'a self,
        context: CapabilityInvocationContext,
        request: AutomationCapabilityRequest,
        _: CapabilityControl,
    ) -> CapabilityFuture<'a> {
        Box::pin(async move {
            crate::authorize_agent_capability(context.agent().unwrap(), &request)?;
            if let AutomationCapabilityRequest::ReadDatabase(request) = &request {
                let state = self.0.lock().unwrap();
                if request
                    .version
                    .as_ref()
                    .is_some_and(|version| version.revision != state.0)
                {
                    return Err(CapabilityFailure::new(
                        CapabilityFailureCode::RevisionConflict,
                    ));
                }
                return Ok(AutomationCapabilityResult::DatabaseRead(
                    DatabaseReadResult {
                        database: request.input.database().clone(),
                        version: ResourceVersion {
                            revision: state.0,
                            session_id: None,
                        },
                        content: DatabaseReadContent::Schema {
                            columns: vec![],
                            page: InspectionPage::known(0, 0, 0),
                        },
                    },
                ));
            }
            let AutomationCapabilityRequest::EditResource(request) = request else {
                panic!("database tools must not add an inspection roundtrip");
            };
            let mut state = self.0.lock().unwrap();
            if request.version.revision != state.0 {
                return Err(CapabilityFailure::new(
                    CapabilityFailureCode::RevisionConflict,
                ));
            }
            let inserted_row_ids = if matches!(request.edit, ResourceEdit::InsertRows { .. }) {
                vec![40, 41]
            } else {
                vec![]
            };
            let item_count = match &request.edit {
                ResourceEdit::InsertRows { rows, .. } => rows.len(),
                ResourceEdit::UpdateCells { cells } => cells.len(),
                ResourceEdit::DeleteRows { row_ids } => row_ids.len(),
                ResourceEdit::CreateColumns { columns } => columns.len(),
                ResourceEdit::RenameColumns { columns } => columns.len(),
                ResourceEdit::DeleteColumns { columns } => columns.len(),
                ResourceEdit::CastColumns { columns } => columns.len(),
                ResourceEdit::SetColumnSemantics { columns } => columns.len(),
                _ => unreachable!(),
            };
            state.0 += 1;
            state.1 += 1;
            Ok(AutomationCapabilityResult::ResourceEdited(
                ResourceMutationReceipt {
                    publication_revision: Some(state.0),
                    changes: vec![ResourceChange {
                        resource: request.resource.clone(),
                        revision: state.0,
                        revision_kind: ResourceRevisionKind::Resource,
                        deleted: false,
                    }],
                    moves: vec![],
                    mind_edit: None,
                    document_edit: None,
                    resources: vec![ResourceMutationState {
                        resource: request.resource,
                        name: "Sample".into(),
                        version: ResourceVersion {
                            revision: state.0,
                            session_id: None,
                        },
                        dirty: Some(true),
                        root_topic_id: None,
                    }],
                    database_edit: Some(DatabaseEditReceipt {
                        item_count,
                        inserted_row_ids,
                        column_names: vec![],
                        dirty: true,
                        can_undo: true,
                        can_redo: false,
                    }),
                },
            ))
        })
    }
}

#[tokio::test]
async fn database_tools_bind_committed_versions_and_keep_public_ledger_arguments() {
    let database = model::DatabaseResourceRef::new("sample".into());
    let resource = database.resource();
    let gateway = Arc::new(DatabaseGateway(Mutex::new((7, 0))));
    let store = Arc::new(InMemoryHarnessStore::default());
    let scope = Arc::new(Mutex::new(AgentInvocationScope {
        run_id: AgentRunId::try_new("data-worker").unwrap(),
        role: AgentRole::Data,
        task: Some(AgentTaskScope {
            resources: vec![AgentResourceAccess {
                resource: resource.clone(),
                version: None,
                operations: vec![
                    AgentResourceOperation::Inspect,
                    AgentResourceOperation::Edit,
                ],
            }],
            ..Default::default()
        }),
    }));
    let run = capabilities::executor(
        gateway.clone(),
        store.clone(),
        Arc::new(SequentialIds::default()),
        scope.clone(),
        ResourceObservations::default(),
    );
    let read = model::CapabilityInput::InspectDatabaseSchema(model::InspectDatabaseSchemaInput {
        database: database.clone(),
        columns: vec![],
        offset: 0,
        limit: 1,
    });
    let first = run
        .execute(ModelCapabilityRequest {
            request: read.clone(),
        })
        .await
        .unwrap();
    let record = store
        .load_invocation(
            &HarnessSessionId::try_new("session").unwrap(),
            &first.invocation_id,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.request.model_input().unwrap(), read);
    assert_eq!(
        run.observations
            .lock()
            .unwrap()
            .version(&resource, false)
            .unwrap()
            .unwrap()
            .revision,
        7
    );
    let visible = model::capability_result(&first.result).unwrap();
    assert!(visible["payload"].get("version").is_none());
    let inputs = [
        model::CapabilityInput::InsertRows(model::InsertRowsInput {
            database: database.clone(),
            rows: vec![
                [("value".into(), serde_json::from_value(json!(5)).unwrap())].into(),
                BTreeMap::new(),
            ],
            before_row_id: Some(3),
        }),
        model::CapabilityInput::UpdateCells(model::UpdateCellsInput {
            database: database.clone(),
            cells: vec![model::DatabaseCellEdit {
                row_id: 40,
                column: "value".into(),
                value: serde_json::from_value(json!(null)).unwrap(),
            }],
        }),
        model::CapabilityInput::DeleteRows(model::DeleteRowsInput {
            database: database.clone(),
            row_ids: vec![40, 41],
        }),
        model::CapabilityInput::CreateColumns(serde_json::from_value(json!({"database":{"kind":"database","id":"sample"},"columns":[{"name":"flag","dtype":"Bool"}]})).unwrap()),
        model::CapabilityInput::RenameColumns(serde_json::from_value(json!({"database":{"kind":"database","id":"sample"},"columns":[{"column":"x","name":"value"}]})).unwrap()),
        model::CapabilityInput::DeleteColumns(serde_json::from_value(json!({"database":{"kind":"database","id":"sample"},"columns":["x"]})).unwrap()),
        model::CapabilityInput::CastColumns(serde_json::from_value(json!({"database":{"kind":"database","id":"sample"},"columns":[{"column":"x","dtype":"Int64","force":false}]})).unwrap()),
        model::CapabilityInput::SetColumnSemantics(serde_json::from_value(json!({"database":{"kind":"database","id":"sample"},"columns":[{"column":"x","semantic":{"kind":"Identifier","values":[],"positiveValue":null,"numeric":null}}]})).unwrap()),
    ];
    for input in &inputs {
        let outcome = run
            .execute(ModelCapabilityRequest {
                request: input.clone(),
            })
            .await
            .unwrap();
        let record = store
            .load_invocation(
                &HarnessSessionId::try_new("session").unwrap(),
                &outcome.invocation_id,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(record.capability_id, input.capability_id());
        assert_eq!(record.request.model_input().unwrap(), *input);
        assert_eq!(record.state, ToolInvocationState::Succeeded);
        let visible = model::capability_result(&outcome.result).unwrap();
        assert!(!visible.to_string().contains("revision"));
        if input.capability_id() == CapabilityId::InsertRows {
            assert_eq!(
                visible["payload"]["databaseEdit"]["insertedRowIds"],
                json!([40, 41])
            );
        }
    }
    assert_eq!(
        scope.lock().unwrap().task.as_ref().unwrap().resources[0]
            .version
            .as_ref()
            .unwrap()
            .revision,
        7 + inputs.len() as u64
    );
    gateway.0.lock().unwrap().0 += 1;
    assert_eq!(
        run.execute(ModelCapabilityRequest { request: read })
            .await
            .unwrap_err()
            .code,
        CapabilityFailureCode::RevisionConflict
    );
    assert_eq!(
        scope.lock().unwrap().task.as_ref().unwrap().resources[0]
            .version
            .as_ref()
            .unwrap()
            .revision,
        7 + inputs.len() as u64
    );
    assert_eq!(
        run.execute(ModelCapabilityRequest {
            request: inputs[1].clone()
        })
        .await
        .unwrap_err()
        .code,
        CapabilityFailureCode::RevisionConflict
    );
    assert_eq!(gateway.0.lock().unwrap().1, inputs.len());
    scope.lock().unwrap().task.as_mut().unwrap().resources[0]
        .operations
        .clear();
    assert_eq!(
        run.execute(ModelCapabilityRequest {
            request: inputs[0].clone()
        })
        .await
        .unwrap_err()
        .code,
        CapabilityFailureCode::InvalidRequest
    );
    assert_eq!(gateway.0.lock().unwrap().1, inputs.len());
}
