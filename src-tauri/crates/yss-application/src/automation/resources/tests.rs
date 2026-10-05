use super::*;
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
                InspectResourceRequest {
                    graph_view: GraphInspectionView::Overview,
                    metadata_only: false,
                    resource: doc,
                    offset: 0,
                    limit: 1
                }
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
        self.page(resource, 0, 100)
    }
    fn page(
        &mut self,
        resource: &ProjectResourceRef,
        offset: usize,
        limit: usize,
    ) -> ResourceInspection {
        let AutomationCapabilityResult::ResourceInspection(result) = self
            .call(AutomationCapabilityRequest::InspectResource(
                InspectResourceRequest {
                    graph_view: GraphInspectionView::Overview,
                    metadata_only: false,
                    resource: resource.clone(),
                    offset,
                    limit,
                },
            ))
            .unwrap()
        else {
            panic!("resource inspection")
        };
        result
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
                yss_ui_contract::RequestUiIntent {
                    client_key: uuid::Uuid::new_v4().to_string(),
                    intent: intent.clone(),
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
        f.inspect(&copied);
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
                    InspectResourceRequest {
                        graph_view: GraphInspectionView::Overview,
                        metadata_only: false,
                        resource: item,
                        offset: 0,
                        limit: 100
                    }
                ))
                .unwrap_err()
                .code,
                CapabilityFailureCode::ResourceUnavailable
            );
        }
    }
    session.presentation.detach_workbench();
    let AutomationCapabilityResult::ProjectInspection(project) = f
        .call(AutomationCapabilityRequest::InspectProject(
            InspectProjectRequest {},
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
fn markdown_pages_and_range_edits_preserve_unseen_unicode_and_reject_stale_versions() {
    let mut f = Fixture::new();
    let resource = f.create(ResourceCreation::Doc {
        name: "Report".into(),
    });
    assert!(
        matches!(f.inspect(&resource).content, ResourceContent::Doc { markdown, .. } if markdown.is_empty())
    );
    f.edit(
        &resource,
        ResourceEdit::Doc {
            operations: vec![MarkdownOperation::SetMarkdown {
                markdown: "A🙂中文Z".into(),
            }],
        },
    );
    let page = f.page(&resource, 1, 2);
    assert!(page.dirty);
    assert!(
        matches!(&page.content, ResourceContent::Doc { markdown, total_characters: 5, next_offset: Some(3) } if markdown == "🙂中")
    );
    f.edit(
        &resource,
        ResourceEdit::Doc {
            operations: vec![MarkdownOperation::ReplaceRange {
                start: 1,
                end: 4,
                markdown: "段落".into(),
            }],
        },
    );
    let publications = f.publications.len();
    assert_eq!(
        f.call(AutomationCapabilityRequest::EditResource(
            EditResourceRequest {
                resource: resource.clone(),
                version: page.version,
                edit: ResourceEdit::Doc {
                    operations: vec![MarkdownOperation::SetMarkdown {
                        markdown: "stale".into()
                    }]
                }
            }
        ))
        .unwrap_err()
        .code,
        CapabilityFailureCode::RevisionConflict
    );
    let current = f.inspect(&resource);
    assert_eq!(
        f.call(AutomationCapabilityRequest::EditResource(
            EditResourceRequest {
                resource: resource.clone(),
                version: current.version,
                edit: ResourceEdit::Doc {
                    operations: vec![MarkdownOperation::ReplaceRange {
                        start: 99,
                        end: 100,
                        markdown: "invalid".into()
                    }]
                }
            }
        ))
        .unwrap_err()
        .code,
        CapabilityFailureCode::InvalidRequest
    );
    assert_eq!(f.publications.len(), publications);
    assert_eq!(
        std::fs::read_to_string(f.directory.join("project").join(&resource.id)).unwrap(),
        ""
    );
    f.save(&resource);
    assert_eq!(
        std::fs::read_to_string(f.directory.join("project").join(&resource.id)).unwrap(),
        "A段落Z"
    );
    assert!(!f.inspect(&resource).dirty);
}

#[test]
fn mind_batches_resolve_created_ids_and_roll_back_invalid_hierarchy_changes() {
    let mut f = Fixture::new();
    let resource = f.create(ResourceCreation::Mind {
        name: "Plan".into(),
    });
    let ResourceContent::Mind { root_id, .. } = f.inspect(&resource).content else {
        panic!()
    };
    let created = f.edit(
        &resource,
        ResourceEdit::Mind {
            operations: vec![
                MindOperation::AddNode {
                    client_id: "branch".into(),
                    parent_id: root_id.clone(),
                    content: "Branch".into(),
                },
                MindOperation::AddNode {
                    client_id: "leaf".into(),
                    parent_id: "$branch".into(),
                    content: "Leaf".into(),
                },
                MindOperation::SetReference {
                    node_id: "$leaf".into(),
                    reference: Some(MindResourceReference::Resource {
                        path: "docs/Report.md".into(),
                    }),
                },
            ],
        },
    );
    let branch = &created.created_nodes["branch"];
    let leaf = &created.created_nodes["leaf"];
    assert_ne!(branch, "branch");
    let before = f.inspect(&resource);
    let published = f.publications.len();
    let error = f
        .call(AutomationCapabilityRequest::EditResource(
            EditResourceRequest {
                resource: resource.clone(),
                version: before.version.clone(),
                edit: ResourceEdit::Mind {
                    operations: vec![
                        MindOperation::SetContent {
                            node_id: root_id.clone(),
                            content: "must not commit".into(),
                        },
                        MindOperation::MoveNode {
                            node_id: branch.clone(),
                            parent_id: leaf.clone(),
                            before_id: None,
                        },
                    ],
                },
            },
        ))
        .unwrap_err();
    assert_eq!(error.code, CapabilityFailureCode::MutationRejected);
    assert_eq!(f.publications.len(), published);
    assert_eq!(f.inspect(&resource), before);
    f.edit(
        &resource,
        ResourceEdit::Mind {
            operations: vec![
                MindOperation::MoveNode {
                    node_id: leaf.clone(),
                    parent_id: root_id.clone(),
                    before_id: Some(branch.clone()),
                },
                MindOperation::RemoveNode {
                    node_id: branch.clone(),
                },
            ],
        },
    );
    let ResourceContent::Mind {
        nodes, total_nodes, ..
    } = f.inspect(&resource).content
    else {
        panic!()
    };
    assert_eq!(total_nodes, 2);
    assert_eq!(nodes[1].id, *leaf);
    assert_eq!(nodes[1].parent_id.as_deref(), Some(root_id.as_str()));
    assert!(nodes[1].reference.is_some());
    f.save(&resource);
    assert!(!f.inspect(&resource).dirty);
}

#[test]
fn chart_edits_persist_and_reject_an_old_baseline_at_the_writer_boundary() {
    let mut f = Fixture::new();
    let resource = f.create(ResourceCreation::Chart {
        name: "Plot".into(),
    });
    let before = f.inspect(&resource);
    let settings = ChartSettings {
        database_id: "sales".into(),
        chart_type: ChartType::Line,
        x: Some("time".into()),
        y: Some("amount".into()),
    };
    f.edit(
        &resource,
        ResourceEdit::Chart {
            settings: settings.clone(),
        },
    );
    let after = f.inspect(&resource);
    assert_eq!(after.content, ResourceContent::Chart { settings });
    assert!(!after.dirty);
    let session = f.application.as_ref().unwrap().capture_session().unwrap();
    let rejected = f.application.as_ref().unwrap().save_chart_resource(
        session.project_instance_id().clone(),
        OperationId::new(),
        chart_path(&resource).unwrap(),
        yss_chart_document::ChartDocument::new("stale"),
        Some(ResourceRevision::new(before.version.revision)),
    );
    assert!(matches!(
        rejected,
        Err(crate::chart::ChartApplicationError::Project(
            yss_project::ProjectOperationError::ResourceRevisionConflict { .. }
        ))
    ));
    assert_eq!(f.inspect(&resource), after);
    let disk: yss_chart_document::ChartDocument = serde_json::from_str(
        &std::fs::read_to_string(f.directory.join("project").join(&resource.id)).unwrap(),
    )
    .unwrap();
    assert_eq!(disk.chart_type, ChartType::Line);
    assert_eq!(disk.encodings.y.as_deref(), Some("amount"));
}

#[test]
fn datasets_keep_rows_types_semantics_and_history_through_copy_export_and_lifecycle() {
    let mut f = Fixture::new();
    let resource = f.dataset();
    let first = f.page(&resource, 0, 2);
    let ResourceContent::Database {
        rows,
        row_ids,
        next_offset,
        ..
    } = &first.content
    else {
        panic!()
    };
    assert_eq!(rows.len(), 2);
    assert_eq!(*next_offset, Some(2));
    f.edit(
        &resource,
        ResourceEdit::Database {
            operation: DatasetOperation::EditCell {
                row: 0,
                column: "x".into(),
                value: serde_json::json!(9),
                row_id: Some(row_ids[0]),
            },
        },
    );
    assert_eq!(
        f.call(AutomationCapabilityRequest::EditResource(
            EditResourceRequest {
                resource: resource.clone(),
                version: first.version,
                edit: ResourceEdit::Database {
                    operation: DatasetOperation::DeleteRows {
                        indices: vec![0],
                        row_ids: None
                    }
                }
            }
        ))
        .unwrap_err()
        .code,
        CapabilityFailureCode::RevisionConflict
    );
    f.edit(
        &resource,
        ResourceEdit::Database {
            operation: DatasetOperation::AddRow { index: Some(1) },
        },
    );
    let ResourceContent::Database { row_ids, .. } = f.inspect(&resource).content else {
        panic!()
    };
    f.edit(
        &resource,
        ResourceEdit::Database {
            operation: DatasetOperation::DeleteRows {
                indices: vec![1],
                row_ids: Some(vec![row_ids[1]]),
            },
        },
    );
    f.edit(
        &resource,
        ResourceEdit::Database {
            operation: DatasetOperation::AddColumn {
                name: "flag".into(),
                dtype: "Int64".into(),
            },
        },
    );
    f.edit(
        &resource,
        ResourceEdit::Database {
            operation: DatasetOperation::RenameColumn {
                old_name: "flag".into(),
                new_name: "group".into(),
            },
        },
    );
    f.edit(
        &resource,
        ResourceEdit::Database {
            operation: DatasetOperation::SetColumnSemantic {
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
            },
        },
    );
    f.edit(
        &resource,
        ResourceEdit::Database {
            operation: DatasetOperation::DeleteColumn {
                name: "group".into(),
            },
        },
    );
    f.edit(
        &resource,
        ResourceEdit::Database {
            operation: DatasetOperation::Undo,
        },
    );
    let restored = f.inspect(&resource);
    f.edit(
        &resource,
        ResourceEdit::Database {
            operation: DatasetOperation::Redo,
        },
    );
    assert!(
        matches!(f.inspect(&resource).content, ResourceContent::Database { schema, .. } if schema.columns.len() == 2)
    );
    f.edit(
        &resource,
        ResourceEdit::Database {
            operation: DatasetOperation::Undo,
        },
    );
    f.edit(
        &resource,
        ResourceEdit::Database {
            operation: DatasetOperation::CastColumn {
                column: "x".into(),
                dtype: "Float64".into(),
                force: false,
            },
        },
    );
    f.save(&resource);
    let current = f.inspect(&resource);
    assert!(!current.dirty);
    let ResourceContent::Database { schema, rows, .. } = &current.content else {
        panic!()
    };
    assert_eq!(schema.columns[0].physical_type, "Float64");
    assert_eq!(rows[0][0], serde_json::json!(9.0));
    assert!(
        schema.columns[2]
            .semantic
            .as_ref()
            .is_some_and(|value| value.kind == DatasetSemanticKind::Binary)
    );
    let duplicate = f.manage(ManageResourceRequest::Duplicate {
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
    let ResourceContent::Database {
        schema: copied_schema,
        rows: copied_rows,
        ..
    } = f.inspect(&copy).content
    else {
        panic!()
    };
    assert_eq!(copied_rows, *rows);
    assert_eq!(copied_schema.columns, schema.columns);
    let output = f.directory.join("export.csv");
    let export = ExportDatasetRequest {
        resource: resource.clone(),
        version: current.version,
        path: output.to_string_lossy().into(),
        format: DatasetExportFormat::Csv,
    };
    assert!(matches!(
        f.call(AutomationCapabilityRequest::ExportDataset(export))
            .unwrap(),
        AutomationCapabilityResult::DatasetExported(_)
    ));
    let exported = std::fs::read_to_string(&output).unwrap();
    assert!(exported.contains("group"));
    assert_eq!(
        f.call(AutomationCapabilityRequest::ExportDataset(
            ExportDatasetRequest {
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
            yss_ui_contract::RequestUiIntent {
                client_key: "database-open".into(),
                intent: yss_ui_contract::UiIntent::OpenResource {
                    resource: resource.clone(),
                    node_id: None
                }
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
    let ResourceContent::GraphPage {
        function: Some(signature),
        ..
    } = &initial.content
    else {
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
    let ResourceContent::GraphPage {
        function: Some(signature),
        ..
    } = &current.content
    else {
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
    let ResourceContent::GraphPage { graph, .. } = f.inspect(&event).content else {
        panic!()
    };
    f.call(AutomationCapabilityRequest::ApplyGraphEdit(
        ApplyGraphEditRequest {
            graph_path: event.id.clone(),
            base_revision: graph.version.revision,
            graph_hash: graph.graph_hash,
            client_key: "constant".into(),
            locale: "en-US".into(),
            operations: vec![GraphEditOperation::CreateConstant {
                name: "Value".into(),
                value: GraphConstantLiteral::Integer(7),
                x: 10.0,
                y: 20.0,
                client_id: Some("value".into()),
            }],
        },
    ))
    .unwrap();
    let ResourceContent::GraphPage { graph, .. } = f.inspect(&event).content else {
        panic!()
    };
    assert_eq!(graph.counts.nodes, 1);
    f.edit(
        &event,
        ResourceEdit::GraphHistory {
            redo: false,
            graph_hash: graph.graph_hash,
        },
    );
    let ResourceContent::GraphPage { graph, .. } = f.inspect(&event).content else {
        panic!()
    };
    assert_eq!(graph.counts.nodes, 0);
    f.edit(
        &event,
        ResourceEdit::GraphHistory {
            redo: true,
            graph_hash: graph.graph_hash,
        },
    );
    assert!(
        matches!(f.inspect(&event).content, ResourceContent::GraphPage { graph, .. } if graph.counts.nodes == 1)
    );
    f.save(&event);
    assert!(!f.inspect(&event).dirty);
}
