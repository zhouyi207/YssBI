use super::*;
use serde_json::json;
use yss_harness_contract::model::*;

struct DocumentGateway(AtomicU64);
impl CapabilityGatewayPort for DocumentGateway {
    fn invoke<'a>(
        &'a self,
        context: CapabilityInvocationContext,
        request: AutomationCapabilityRequest,
        _: CapabilityControl,
    ) -> CapabilityFuture<'a> {
        Box::pin(async move {
            crate::authorize_agent_capability(context.agent().unwrap(), &request)?;
            let current = version(self.0.load(Ordering::Acquire));
            let conflict = || CapabilityFailure::new(CapabilityFailureCode::RevisionConflict);
            match request {
                AutomationCapabilityRequest::ReadDocument(request) => {
                    if request
                        .version
                        .as_ref()
                        .is_some_and(|expected| expected != &current)
                    {
                        return Err(conflict());
                    }
                    let section = DocumentSectionRef {
                        heading_index: 0,
                        start: 0,
                    };
                    let content = match request.input {
                        DocumentReadInput::Outline(_) => DocumentReadContent::Outline {
                            character_count: 20,
                            heading_count: 1,
                            headings: vec![DocumentHeading {
                                section,
                                title: "Results".into(),
                                title_complete: true,
                                level: 1,
                                parent_heading_index: None,
                                body_start: 10,
                                end: 20,
                            }],
                            page: InspectionPage::known(0, 1, 1),
                        },
                        DocumentReadInput::Text(value) => DocumentReadContent::Text {
                            markdown: "# Results\nCurrent".into(),
                            range: DocumentCharacterRange { start: 0, end: 17 },
                            scope: DocumentCharacterRange { start: 0, end: 20 },
                            page: InspectionPage::known(0, 17, 20),
                            complete: false,
                            section: value.section,
                        },
                        DocumentReadInput::Search(_) => DocumentReadContent::Search {
                            matches: vec![DocumentSearchMatch {
                                range: DocumentCharacterRange { start: 10, end: 17 },
                                context: "Current".into(),
                                context_range: DocumentCharacterRange { start: 10, end: 17 },
                                section: Some(section),
                            }],
                            page: InspectionPage::known(0, 1, 1),
                        },
                    };
                    Ok(AutomationCapabilityResult::DocumentRead(
                        DocumentReadResult {
                            document: DocumentResourceRef::new(document().id),
                            version: current,
                            dirty: true,
                            content,
                        },
                    ))
                }
                AutomationCapabilityRequest::EditResource(request) => {
                    if request.version != current {
                        return Err(conflict());
                    }
                    let revision = self.0.fetch_add(1, Ordering::AcqRel) + 1;
                    Ok(AutomationCapabilityResult::ResourceEdited(
                        ResourceMutationReceipt {
                            publication_revision: Some(revision),
                            changes: vec![ResourceChange {
                                resource: document(),
                                revision,
                                revision_kind: ResourceRevisionKind::Resource,
                                deleted: false,
                            }],
                            moves: vec![],
                            mind_edit: None,
                            database_edit: None,
                            document_edit: Some(DocumentEditReceipt {
                                changes: vec![],
                                character_count: 20,
                                dirty: true,
                            }),
                            resources: vec![ResourceMutationState {
                                resource: document(),
                                name: "Report".into(),
                                version: version(revision),
                                dirty: Some(true),
                                root_topic_id: None,
                            }],
                        },
                    ))
                }
                _ => panic!("unexpected document tool"),
            }
        })
    }
}

#[tokio::test]
async fn document_sections_keep_their_original_read_baseline_through_writes_and_history_replay() {
    let gateway = Arc::new(DocumentGateway(AtomicU64::new(1)));
    let store = Arc::new(InMemoryHarnessStore::default());
    let ids = Arc::new(SequentialIds::default());
    let scope = Arc::new(Mutex::new(AgentInvocationScope {
        run_id: AgentRunId::try_new("document-worker").unwrap(),
        role: AgentRole::Report,
        task: Some(AgentTaskScope {
            resources: vec![AgentResourceAccess {
                resource: document(),
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
        ids.clone(),
        scope.clone(),
        ResourceObservations::default(),
    );
    let document = DocumentResourceRef::new(document().id);
    let outline = CapabilityInput::InspectDocument(
        serde_json::from_value(json!({"document":document})).unwrap(),
    );
    let section = DocumentSectionRef {
        heading_index: 0,
        start: 0,
    };
    let text = CapabilityInput::ReadDocument(
        serde_json::from_value(json!({"document":document,"section":section})).unwrap(),
    );
    let unread = run
        .execute(ModelCapabilityRequest {
            request: text.clone(),
        })
        .await
        .unwrap_err();
    let visible = model::failure(&unread);
    assert_eq!(visible["code"], "resource_read_required");
    assert_eq!(visible["details"]["resourceId"], document.id);
    assert!(
        visible["details"]["nextStep"]
            .as_str()
            .unwrap()
            .contains("outline")
    );
    for internal in ["revision", "session", "baseline"] {
        assert!(!visible.to_string().contains(internal));
    }
    let inputs = [
        outline.clone(),
        text.clone(),
        CapabilityInput::SearchDocument(
            serde_json::from_value(json!({"document":document,"query":"Current"})).unwrap(),
        ),
        CapabilityInput::WriteDocument(WriteDocumentInput {
            document: document.clone(),
            markdown: "Long report paragraph.\n".repeat(5000),
        }),
        CapabilityInput::AppendDocument(AppendDocumentInput {
            document: document.clone(),
            text: "\nConclusion".into(),
        }),
        CapabilityInput::ReplaceDocumentText(ReplaceDocumentTextInput {
            document: document.clone(),
            replacements: vec![DocumentTextReplacement {
                old_text: "Conclusion".into(),
                new_text: "Conclusions".into(),
            }],
        }),
    ];
    let mut replay = ResourceObservations::default();
    for input in inputs {
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
        assert_eq!(record.request.model_input().unwrap(), input);
        assert!(
            !model::capability_result(&outcome.result)
                .unwrap()
                .to_string()
                .contains("revision")
        );
        replay.replay(&record, &record.project);
    }
    assert_eq!(
        scope.lock().unwrap().task.as_ref().unwrap().resources[0]
            .version
            .as_ref()
            .unwrap()
            .revision,
        4
    );
    // The same business locator still exists, but was returned by an older document read.
    assert_eq!(
        replay
            .document_section_version(&document.id, &section)
            .unwrap()
            .revision,
        1
    );
    let restored = capabilities::executor(gateway.clone(), store, ids, scope.clone(), replay);
    let stale = restored
        .execute(ModelCapabilityRequest {
            request: text.clone(),
        })
        .await
        .unwrap_err();
    assert_eq!(stale.code, CapabilityFailureCode::RevisionConflict);
    assert_eq!(stale.details["reason"], "document_section_changed");
    let visible = model::failure(&stale);
    assert_eq!(visible["details"]["resourceId"], document.id);
    assert!(
        visible["details"]["nextStep"]
            .as_str()
            .unwrap()
            .contains("outline")
    );
    restored
        .execute(ModelCapabilityRequest { request: outline })
        .await
        .unwrap();
    restored
        .execute(ModelCapabilityRequest {
            request: text.clone(),
        })
        .await
        .unwrap();
    gateway.0.fetch_add(1, Ordering::AcqRel);
    assert_eq!(
        restored
            .execute(ModelCapabilityRequest { request: text })
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
        4
    );
}
