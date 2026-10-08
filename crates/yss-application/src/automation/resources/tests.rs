use super::*;
use std::collections::BTreeMap;
mod chart;
mod database_reads;
mod document;
mod mind;
use crate::session::{ApplicationSessionEpoch, ApplicationSessionSlot};
use std::{path::PathBuf, sync::Arc, time::Duration};

struct Fixture {
    directory: PathBuf,
    application: Option<ApplicationState>,
    context: CapabilityInvocationContext,
    publications: Vec<CommittedResourceMutation>,
}

#[test]
fn message_references_use_project_identity_and_reject_missing_or_foreign_resources() {
    let mut fixture = Fixture::new();
    let doc = fixture.create(ResourceCreation::Doc {
        name: "Methods".into(),
    });
    let graph = fixture.create(ResourceCreation::EventGraph {
        name: "Analysis".into(),
    });
    let application = fixture.application.as_ref().unwrap().clone();
    let binding = fixture.context.project().clone();
    let references = application
        .resolve_harness_resources(&binding, &[doc.clone(), graph.clone()])
        .unwrap();
    assert_eq!(
        references
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        ["Methods", "Analysis"]
    );
    assert_eq!(references[0].resource, doc);
    assert_eq!(
        serde_json::to_value(&references[0]).unwrap(),
        serde_json::json!({"resource": doc, "name": "Methods"})
    );
    assert_eq!(
        application
            .resolve_harness_resources(&binding, &[doc.clone(), doc.clone()])
            .unwrap_err()
            .code,
        CapabilityFailureCode::InvalidRequest
    );
    let foreign = ProjectSessionBinding::new(
        yss_project_identity::ProjectInstanceId::from_existing("other-project".into()),
        binding.project_session_id().clone(),
    );
    assert_eq!(
        application
            .resolve_harness_resources(&foreign, std::slice::from_ref(&graph))
            .unwrap_err()
            .code,
        CapabilityFailureCode::ProjectSessionChanged
    );
    let version = fixture.inspect(&doc).version;
    fixture.manage(ManageResourceRequest::Delete {
        resource: doc.clone(),
        version,
    });
    assert_eq!(
        application
            .resolve_harness_resources(&binding, &[doc])
            .unwrap_err()
            .code,
        CapabilityFailureCode::ResourceUnavailable
    );
    assert_eq!(
        application
            .resolve_harness_resources(&binding, &[graph])
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn agent_permissions_reject_wrong_resource_kind_and_unassigned_documents_at_gateway() {
    let mut fixture = Fixture::new();
    let doc = fixture.create(ResourceCreation::Doc {
        name: "Report".into(),
    });
    let other = fixture.create(ResourceCreation::Doc {
        name: "Other".into(),
    });
    let chart = fixture.create(ResourceCreation::Chart {
        name: "Plot".into(),
    });
    let doc_version = fixture.inspect(&doc).version;
    let other_version = fixture.inspect(&other).version;
    let chart_version = fixture.inspect(&chart).version;
    let scope = AgentInvocationScope {
        run_id: AgentRunId::try_new("report-run").unwrap(),
        role: AgentRole::Report,
        task: Some(AgentTaskScope {
            resources: vec![
                AgentResourceAccess {
                    resource: doc.clone(),
                    version: Some(doc_version.clone()),
                    operations: vec![
                        AgentResourceOperation::Inspect,
                        AgentResourceOperation::Save,
                    ],
                },
                // Even an overbroad task cannot give ReportAgent a different role's write authority.
                AgentResourceAccess {
                    resource: chart.clone(),
                    version: Some(chart_version.clone()),
                    operations: vec![
                        AgentResourceOperation::Inspect,
                        AgentResourceOperation::Save,
                    ],
                },
            ],
            ..Default::default()
        }),
    };
    fixture.context = fixture.context.clone().with_agent(scope.clone());
    for (resource, version) in [(chart, chart_version), (other, other_version)] {
        let failure = fixture
            .call(AutomationCapabilityRequest::ManageResource(
                ManageResourceRequest::Save { resource, version },
            ))
            .unwrap_err();
        assert_eq!(failure.code, CapabilityFailureCode::InvalidRequest);
    }
    fixture
        .call(AutomationCapabilityRequest::ManageResource(
            ManageResourceRequest::Save {
                resource: doc.clone(),
                version: doc_version.clone(),
            },
        ))
        .unwrap();
    fixture.context = fixture.context.clone().with_agent(AgentInvocationScope {
        role: AgentRole::Review,
        ..scope
    });
    assert!(
        fixture
            .call(AutomationCapabilityRequest::ManageResource(
                ManageResourceRequest::Save {
                    resource: doc.clone(),
                    version: doc_version
                }
            ))
            .is_err()
    );
    assert!(
        fixture
            .call(AutomationCapabilityRequest::InspectResource(
                InspectResourceRequest { resource: doc }
            ))
            .is_ok()
    );
}
impl Fixture {
    fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("harness-resources-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let project = Arc::new(yss_project::ProjectState::new());
        let created = project
            .create_project_transaction("Resources", &directory.join("project"), OperationId::new())
            .unwrap();
        project
            .activate_project_from_path(&created.metadata_path)
            .unwrap();
        let components = crate::session::NodeComponents::builtins().unwrap();
        let candidate = crate::session::build_current_project_candidate(
            ApplicationSessionEpoch::INITIAL,
            project,
            [],
            &components,
        )
        .unwrap();
        let application = ApplicationState::new(Arc::new(ApplicationSessionSlot::new(components)));
        application.install_candidate(candidate).unwrap();
        let session = application.capture_session().unwrap();
        let context = CapabilityInvocationContext::new(
            PrincipalId::try_new("resource-user").unwrap(),
            HarnessSessionId::try_new("resource-session").unwrap(),
            CapabilityInvocationId::try_new("resource-invocation").unwrap(),
            ProjectSessionBinding::new(
                session.project_instance_id().clone(),
                session.project_session_id().clone(),
            ),
        );
        Self {
            directory,
            application: Some(application),
            context,
            publications: Vec::new(),
        }
    }
    fn call(&mut self, request: AutomationCapabilityRequest) -> Result<AutomationCapabilityResult> {
        self.application
            .as_ref()
            .unwrap()
            .invoke_automation_capability(
                self.context.clone(),
                request,
                &CapabilityControl::new(CancellationToken::default(), Duration::from_secs(30)),
                &mut |mutation| self.publications.push(mutation.clone()),
            )
    }
    fn manage(&mut self, request: ManageResourceRequest) -> ResourceMutationReceipt {
        let AutomationCapabilityResult::ResourceManaged(result) = self
            .call(AutomationCapabilityRequest::ManageResource(request))
            .unwrap()
        else {
            panic!("resource receipt")
        };
        if let Some(revision) = result.publication_revision {
            assert_eq!(
                self.publications.last().unwrap().publication_revision,
                revision
            );
        }
        result
    }
    fn create(&mut self, specification: ResourceCreation) -> ProjectResourceRef {
        self.manage(ManageResourceRequest::Create { specification })
            .changes
            .into_iter()
            .find(|change| {
                !change.deleted && change.revision_kind == ResourceRevisionKind::Resource
            })
            .unwrap()
            .resource
    }
    fn inspect(&mut self, resource: &ProjectResourceRef) -> ResourceInspection {
        let AutomationCapabilityResult::ResourceInspection(result) = self
            .call(AutomationCapabilityRequest::InspectResource(
                InspectResourceRequest {
                    resource: resource.clone(),
                },
            ))
            .unwrap()
        else {
            panic!("resource inspection")
        };
        result
    }
    fn graph(&mut self, resource: &ProjectResourceRef) -> GraphInspectionPage {
        let AutomationCapabilityResult::GraphInspectionPage(value) = self
            .call(AutomationCapabilityRequest::InspectGraph(
                InspectGraphRequest::overview(&resource.id),
            ))
            .unwrap()
        else {
            panic!("graph overview")
        };
        value
    }
    fn database_schema(&mut self, resource: &ProjectResourceRef) -> Vec<DatasetColumnSchema> {
        let value = database_reads::read(
            self,
            model::DatabaseReadInput::Schema(model::InspectDatabaseSchemaInput {
                database: model::DatabaseResourceRef::new(resource.id.clone()),
                columns: vec![],
                offset: 0,
                limit: 100,
            }),
            None,
        )
        .unwrap();
        let DatabaseReadContent::Schema { columns, .. } = value.content else {
            panic!("database schema")
        };
        columns
    }
    fn database_rows(&mut self, resource: &ProjectResourceRef, limit: usize) -> DatabaseReadResult {
        let columns = self
            .database_schema(resource)
            .into_iter()
            .map(|column| column.name)
            .collect();
        database_reads::read(
            self,
            model::DatabaseReadInput::Rows(model::ReadDatabaseRowsInput {
                database: model::DatabaseResourceRef::new(resource.id.clone()),
                columns,
                filters: vec![],
                order: vec![],
                offset: 0,
                limit,
            }),
            None,
        )
        .unwrap()
    }
    fn edit(
        &mut self,
        resource: &ProjectResourceRef,
        edit: ResourceEdit,
    ) -> ResourceMutationReceipt {
        let version = self.inspect(resource).version;
        let AutomationCapabilityResult::ResourceEdited(result) = self
            .call(AutomationCapabilityRequest::EditResource(
                EditResourceRequest {
                    resource: resource.clone(),
                    version,
                    edit,
                },
            ))
            .unwrap()
        else {
            panic!("edit receipt")
        };
        result
    }
    fn save(&mut self, resource: &ProjectResourceRef) {
        let version = self.inspect(resource).version;
        self.manage(ManageResourceRequest::Save {
            resource: resource.clone(),
            version,
        });
    }
    fn dataset(&mut self) -> ProjectResourceRef {
        let csv = self.directory.join("Data.csv");
        std::fs::write(&csv, "x,label\n1,a\n2,b\n3,c\n").unwrap();
        self.create(ResourceCreation::Database {
            name: None,
            source: DatasetImportSource::Csv {
                path: csv.to_string_lossy().into(),
                delimiter: ',',
                has_header: true,
                infer_schema_length: Some(10),
            },
        })
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.application.take();
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn publication_failure_keeps_the_committed_resource_receipt() {
    let mut f = Fixture::new();
    let result = f
        .application
        .as_ref()
        .unwrap()
        .invoke_automation_capability(
            f.context.clone(),
            AutomationCapabilityRequest::ManageResource(ManageResourceRequest::Create {
                specification: ResourceCreation::Doc {
                    name: "Committed".into(),
                },
            }),
            &CapabilityControl::new(CancellationToken::default(), Duration::from_secs(30)),
            &mut |_| panic!("injected notification failure"),
        )
        .unwrap();
    let AutomationCapabilityResult::ResourceManaged(receipt) = result else {
        panic!()
    };
    assert!(receipt.publication_revision.is_some());
    let resource = &receipt.changes[0].resource;
    assert_eq!(f.inspect(resource).name, "Committed");
    assert!(f.directory.join("project").join(&resource.id).exists());
}

#[test]
fn lifecycle_receipts_continue_at_the_committed_identity_without_content_reads() {
    let mut f = Fixture::new();
    let created = f.manage(ManageResourceRequest::Create {
        specification: ResourceCreation::Mind {
            name: "Ideas".into(),
        },
    });
    let state = created.resources.into_iter().next().unwrap();
    assert_eq!(state.dirty, Some(false));
    assert!(state.root_topic_id.is_some());
    let renamed = f.manage(ManageResourceRequest::Rename {
        resource: state.resource,
        version: state.version,
        name: "Research".into(),
    });
    let state = renamed.resources[0].clone();
    assert_eq!(state.name, "Research");
    assert_eq!(state.resource.id, "minds/Research.yssbi-mind");
    let copied = f.manage(ManageResourceRequest::Duplicate {
        resource: state.resource.clone(),
        version: state.version.clone(),
        name: Some("Research copy".into()),
    });
    let copy = copied.resources.into_iter().next().unwrap();
    assert_eq!(copy.name, "Research copy");
    assert_ne!(state.resource, copy.resource);
    assert_ne!(state.root_topic_id, copy.root_topic_id);
    let failed = f
        .call(AutomationCapabilityRequest::ManageResource(
            ManageResourceRequest::Duplicate {
                resource: state.resource.clone(),
                version: state.version.clone(),
                name: Some("../invalid".into()),
            },
        ))
        .unwrap_err();
    assert_eq!(failed.code, CapabilityFailureCode::MutationRejected);
    let saved = f.manage(ManageResourceRequest::Save {
        resource: copy.resource,
        version: copy.version,
    });
    let copy = saved.resources.into_iter().next().unwrap();
    let deleted = f.manage(ManageResourceRequest::Delete {
        resource: copy.resource.clone(),
        version: copy.version,
    });
    assert!(
        deleted
            .changes
            .iter()
            .any(|change| change.resource == copy.resource && change.deleted)
    );
    assert!(deleted.resources.is_empty());
    let public =
        model::capability_result(&AutomationCapabilityResult::ResourceManaged(renamed)).unwrap();
    assert_eq!(public["payload"]["resources"][0]["name"], "Research");
    assert!(public["payload"]["resources"][0].get("version").is_none());
}

#[test]
fn resource_catalog_pages_filters_and_inspects_without_decoding_bodies() {
    let mut f = Fixture::new();
    let a = f.create(ResourceCreation::Doc {
        name: "Report A".into(),
    });
    f.create(ResourceCreation::Doc {
        name: "Report B".into(),
    });
    let graph = f.create(ResourceCreation::EventGraph {
        name: "Report graph".into(),
    });
    let session = f.application.as_ref().unwrap().capture_session().unwrap();
    let root = session.project().capture_project_session().unwrap().root;
    // Metadata remains available even when a file body cannot be decoded. Only
    // subsequent content use owns that validation; discovery must not open it.
    std::fs::write(root.as_path().join(&graph.id), "invalid graph body").unwrap();
    std::fs::write(root.as_path().join(&a.id), [0xff, 0xfe]).unwrap();
    let request = ListResourcesRequest {
        kinds: vec![ProjectResourceKind::Doc],
        query: Some("REPORT".into()),
        limit: 1,
        ..Default::default()
    };
    let AutomationCapabilityResult::ProjectInspection(first) = f
        .call(AutomationCapabilityRequest::ListResources(request.clone()))
        .unwrap()
    else {
        panic!("resource list");
    };
    assert_eq!(first.resources.len(), 1);
    assert_eq!(first.resources[0].resource, a);
    assert_eq!(first.page, InspectionPage::known(0, 1, 2));
    let AutomationCapabilityResult::ProjectInspection(second) = f
        .call(AutomationCapabilityRequest::ListResources(
            ListResourcesRequest {
                offset: first.page.next_offset.unwrap(),
                ..request
            },
        ))
        .unwrap()
    else {
        panic!("resource list");
    };
    assert!(!second.page.has_more);
    assert_ne!(first.resources[0].resource, second.resources[0].resource);
    for resource in [a, graph] {
        let AutomationCapabilityResult::ResourceInspection(metadata) = f
            .call(AutomationCapabilityRequest::InspectResource(
                InspectResourceRequest {
                    resource: resource.clone(),
                },
            ))
            .unwrap()
        else {
            panic!("resource metadata");
        };
        assert_eq!(metadata.resource, resource);
        assert!(matches!(metadata.content, ResourceContent::Metadata));
    }
}

#[test]
fn file_lifecycles_return_real_identities_publish_and_open_each_resource_kind() {
    let mut f = Fixture::new();
    let session = f.application.as_ref().unwrap().capture_session().unwrap();
    session.presentation.attach_workbench();
    for specification in [
        ResourceCreation::EventGraph {
            name: "Event".into(),
        },
        ResourceCreation::FunctionGraph {
            name: "Function".into(),
        },
        ResourceCreation::Chart {
            name: "Chart".into(),
        },
        ResourceCreation::Mind {
            name: "Mind".into(),
        },
        ResourceCreation::Doc { name: "Doc".into() },
    ] {
        let resource = f.create(specification);
        let original = f.inspect(&resource);
        let intent = yss_ui_contract::UiIntent::OpenResource {
            resource: resource.clone(),
            node_id: None,
        };
        let result = f
            .call(AutomationCapabilityRequest::RequestUiIntent(
                RequestUiIntent {
                    client_key: uuid::Uuid::new_v4().to_string(),
                    input: model::RequestUiIntentInput {
                        intent: model::UiIntentInput::try_from(&intent).unwrap(),
                    },
                },
            ))
            .unwrap();
        assert!(
            matches!(result, AutomationCapabilityResult::UiIntentReceipt(receipt) if receipt.intent == intent && receipt.status == yss_ui_contract::UiIntentStatus::Pending)
        );
        let renamed = f.manage(ManageResourceRequest::Rename {
            resource: resource.clone(),
            version: original.version,
            name: format!("{} Renamed", original.name),
        });
        let target = renamed
            .moves
            .iter()
            .find(|entry| entry.from == resource)
            .unwrap()
            .to
            .clone();
        assert_eq!(target.kind, resource.kind);
        let version = f.inspect(&target).version;
        let copy = f.manage(ManageResourceRequest::Duplicate {
            name: Some(format!("{} Custom copy", original.name)),
            resource: target.clone(),
            version,
        });
        let copied = copy
            .changes
            .iter()
            .find(|change| {
                change.resource != target
                    && !change.deleted
                    && change.revision_kind == ResourceRevisionKind::Resource
            })
            .unwrap()
            .resource
            .clone();
        assert_eq!(copied.kind, target.kind);
        assert_eq!(
            f.inspect(&copied).name,
            format!("{} Custom copy", original.name)
        );
        f.save(&target);
        for item in [target, copied] {
            let version = f.inspect(&item).version;
            let removed = f.manage(ManageResourceRequest::Delete {
                resource: item.clone(),
                version,
            });
            assert!(
                removed
                    .changes
                    .iter()
                    .any(|change| change.resource == item && change.deleted)
            );
            assert_eq!(
                f.call(AutomationCapabilityRequest::InspectResource(
                    InspectResourceRequest { resource: item }
                ))
                .unwrap_err()
                .code,
                CapabilityFailureCode::ResourceUnavailable
            );
        }
    }
    session.presentation.detach_workbench();
    let AutomationCapabilityResult::ProjectInspection(project) = f
        .call(AutomationCapabilityRequest::ListResources(
            ListResourcesRequest::default(),
        ))
        .unwrap()
    else {
        panic!()
    };
    assert!(project.resources.is_empty());
    assert!(
        f.publications
            .windows(2)
            .all(|pair| pair[0].publication_revision < pair[1].publication_revision)
    );
}

#[test]
fn datasets_keep_rows_types_semantics_and_history_through_copy_export_and_lifecycle() {
    let mut f = Fixture::new();
    let resource = f.dataset();
    let first = f.database_rows(&resource, 2);
    let DatabaseReadContent::Rows {
        rows,
        row_ids,
        page,
        ..
    } = &first.content
    else {
        panic!()
    };
    assert_eq!(rows.len(), 2);
    assert_eq!(page.next_offset, Some(2));
    f.edit(
        &resource,
        ResourceEdit::UpdateCells {
            cells: vec![model::DatabaseCellEdit {
                row_id: row_ids[0],
                column: "x".into(),
                value: yss_data_contract::TabularScalar::Integer(9),
            }],
        },
    );
    assert_eq!(
        f.call(AutomationCapabilityRequest::EditResource(
            EditResourceRequest {
                resource: resource.clone(),
                version: first.version,
                edit: ResourceEdit::DeleteRows {
                    row_ids: vec![row_ids[0]],
                }
            }
        ))
        .unwrap_err()
        .code,
        CapabilityFailureCode::RevisionConflict
    );
    let inserted = f.edit(
        &resource,
        ResourceEdit::InsertRows {
            rows: vec![
                BTreeMap::new(),
                [("x".into(), yss_data_contract::TabularScalar::Integer(15))].into(),
            ],
            before_row_id: Some(row_ids[1]),
        },
    );
    let inserted_rows = inserted.database_edit.as_ref().unwrap();
    assert_eq!(inserted_rows.item_count, 2);
    assert_eq!(inserted_rows.inserted_row_ids.len(), 2);
    assert!(inserted_rows.dirty && inserted_rows.can_undo);
    let visible = model::capability_result(&AutomationCapabilityResult::ResourceEdited(
        inserted.clone(),
    ))
    .unwrap();
    assert_eq!(
        visible["payload"]["databaseEdit"]["insertedRowIds"],
        serde_json::json!(inserted_rows.inserted_row_ids)
    );
    let DatabaseReadContent::Rows { row_ids, .. } = f.database_rows(&resource, 100).content else {
        panic!()
    };
    f.edit(
        &resource,
        ResourceEdit::DeleteRows {
            row_ids: vec![row_ids[1], row_ids[2]],
        },
    );
    f.edit(
        &resource,
        ResourceEdit::CreateColumns {
            columns: vec![model::DatabaseColumnDeclaration {
                name: "flag".into(),
                dtype: "Int64".into(),
            }],
        },
    );
    f.edit(
        &resource,
        ResourceEdit::RenameColumns {
            columns: vec![model::DatabaseColumnRename {
                column: "flag".into(),
                name: "group".into(),
            }],
        },
    );
    f.edit(
        &resource,
        ResourceEdit::SetColumnSemantics {
            columns: vec![model::DatabaseColumnMeaning {
                column: "group".into(),
                semantic: DatasetColumnSemantic {
                    kind: DatasetSemanticKind::Binary,
                    values: vec![
                        DatasetSemanticValue {
                            value: "0".into(),
                            label: "No".into(),
                        },
                        DatasetSemanticValue {
                            value: "1".into(),
                            label: "Yes".into(),
                        },
                    ],
                    positive_value: Some("1".into()),
                    numeric: None,
                },
            }],
        },
    );
    f.edit(
        &resource,
        ResourceEdit::DeleteColumns {
            columns: vec!["group".into()],
        },
    );
    f.edit(&resource, ResourceEdit::DatabaseHistory { redo: false });
    let restored = f.inspect(&resource);
    f.edit(&resource, ResourceEdit::DatabaseHistory { redo: true });
    assert!(f.database_schema(&resource).len() == 2);
    f.edit(&resource, ResourceEdit::DatabaseHistory { redo: false });
    f.edit(
        &resource,
        ResourceEdit::CastColumns {
            columns: vec![model::DatabaseColumnCast {
                column: "x".into(),
                dtype: "Float64".into(),
                force: false,
            }],
        },
    );
    f.save(&resource);
    let current = f.inspect(&resource);
    assert!(!current.dirty);
    let schema = f.database_schema(&resource);
    let page = f.database_rows(&resource, 100);
    let DatabaseReadContent::Rows { rows, .. } = &page.content else {
        panic!()
    };
    assert_eq!(schema[0].physical_type, "Float64");
    assert_eq!(
        serde_json::to_value(&rows[0][0]).unwrap(),
        serde_json::json!(9.0)
    );
    assert!(
        schema[2]
            .semantic
            .as_ref()
            .is_some_and(|value| value.kind == DatasetSemanticKind::Binary)
    );
    let duplicate = f.manage(ManageResourceRequest::Duplicate {
        name: None,
        resource: resource.clone(),
        version: current.version.clone(),
    });
    let copy = duplicate
        .changes
        .iter()
        .find(|change| !change.deleted && change.resource != resource)
        .unwrap()
        .resource
        .clone();
    let copied_schema = f.database_schema(&copy);
    let DatabaseReadContent::Rows {
        rows: copied_rows, ..
    } = f.database_rows(&copy, 100).content
    else {
        panic!()
    };
    assert_eq!(copied_rows, *rows);
    assert_eq!(copied_schema, schema);
    let output = f.directory.join("export.csv");
    let export = ExportDatabaseRequest {
        resource: resource.clone(),
        version: current.version,
        path: output.to_string_lossy().into(),
        format: DatasetExportFormat::Csv,
    };
    assert!(matches!(
        f.call(AutomationCapabilityRequest::ExportDatabase(export))
            .unwrap(),
        AutomationCapabilityResult::DatabaseExported(_)
    ));
    let exported = std::fs::read_to_string(&output).unwrap();
    assert!(exported.contains("group"));
    assert_eq!(
        f.call(AutomationCapabilityRequest::ExportDatabase(
            ExportDatabaseRequest {
                resource: resource.clone(),
                version: restored.version,
                path: output.to_string_lossy().into(),
                format: DatasetExportFormat::Csv
            }
        ))
        .unwrap_err()
        .code,
        CapabilityFailureCode::RevisionConflict
    );
    assert_eq!(std::fs::read_to_string(&output).unwrap(), exported);
    let version = f.inspect(&resource).version;
    f.manage(ManageResourceRequest::Rename {
        resource: resource.clone(),
        version,
        name: "Renamed Data".into(),
    });
    assert_eq!(f.inspect(&resource).name, "Renamed Data");
    let session = f.application.as_ref().unwrap().capture_session().unwrap();
    session.presentation.attach_workbench();
    assert!(
        f.call(AutomationCapabilityRequest::RequestUiIntent(
            RequestUiIntent {
                client_key: "database-open".into(),
                input: model::RequestUiIntentInput {
                    intent: model::UiIntentInput::OpenResource {
                        resource: resource.clone(),
                        node_id: None,
                    },
                },
            }
        ))
        .is_ok()
    );
    session.presentation.detach_workbench();
    for item in [resource, copy] {
        let version = f.inspect(&item).version;
        let removed = f.manage(ManageResourceRequest::Delete {
            resource: item.clone(),
            version,
        });
        assert!(
            removed
                .changes
                .iter()
                .any(|change| change.resource == item && change.deleted)
        );
    }
}

#[test]
fn function_signatures_and_graph_history_share_the_current_project_editing_state() {
    let mut f = Fixture::new();
    let function = f.create(ResourceCreation::FunctionGraph {
        name: "Compute".into(),
    });
    let initial = f.inspect(&function);
    let ResourceContent::Function { signature, .. } = &initial.content else {
        panic!()
    };
    let numeric =
        yss_data_contract::ValueType::Scalar(yss_data_contract::SemanticType::Numeric).to_string();
    f.edit(
        &function,
        ResourceEdit::FunctionSignature {
            signature: FunctionSignatureInspection {
                revision: signature.revision,
                parameters: vec![FunctionParameterInspection {
                    id: None,
                    name: "Input".into(),
                    type_name: numeric.clone(),
                }],
                return_type: Some(numeric),
            },
        },
    );
    let current = f.inspect(&function);
    let ResourceContent::Function { signature, .. } = &current.content else {
        panic!()
    };
    assert!(
        signature.parameters[0]
            .id
            .as_ref()
            .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok())
    );
    assert_eq!(
        f.call(AutomationCapabilityRequest::EditResource(
            EditResourceRequest {
                resource: function,
                version: initial.version,
                edit: ResourceEdit::FunctionSignature {
                    signature: FunctionSignatureInspection {
                        revision: 0,
                        parameters: vec![],
                        return_type: None
                    }
                }
            }
        ))
        .unwrap_err()
        .code,
        CapabilityFailureCode::RevisionConflict
    );
    let event = f.create(ResourceCreation::EventGraph {
        name: "Main".into(),
    });
    let graph = f.graph(&event);
    f.call(AutomationCapabilityRequest::ApplyGraphEdit(
        ApplyGraphEditRequest {
            graph_path: event.id.clone(),
            base_revision: graph.version.revision,
            graph_hash: graph.graph_hash,
            client_key: "constant".into(),
            locale: "en-US".into(),
            operations: vec![GraphEditOperation::CreateConstant {
                declaration: ConstantDeclaration {
                    client_id: "value".into(),
                    name: "Value".into(),
                    value: ConstantValueInput {
                        data_type: yss_data_contract::ValueType::Scalar(
                            yss_data_contract::SemanticType::Numeric,
                        ),
                        data_value: yss_data_contract::DataValue::Integer(7),
                        tabular: None,
                    },
                    description: String::new(),
                    tags: vec![],
                    reference_node: Some(ConstantReferenceNode {
                        client_id: None,
                        position: model::NodePositionInput { x: 10., y: 20. },
                        label: None,
                    }),
                },
            }],
        },
    ))
    .unwrap();
    let graph = f.graph(&event);
    assert_eq!(graph.counts.nodes, 1);
    f.edit(
        &event,
        ResourceEdit::GraphHistory {
            redo: false,
            graph_hash: graph.graph_hash,
        },
    );
    let graph = f.graph(&event);
    assert_eq!(graph.counts.nodes, 0);
    f.edit(
        &event,
        ResourceEdit::GraphHistory {
            redo: true,
            graph_hash: graph.graph_hash,
        },
    );
    assert!(f.graph(&event).counts.nodes == 1);
    f.save(&event);
    assert!(!f.inspect(&event).dirty);
}
