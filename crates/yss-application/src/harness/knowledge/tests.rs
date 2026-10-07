use super::*;
use crate::session::{ApplicationSessionEpoch, ApplicationSessionSlot, NodeComponents};
use yss_harness_core::test_support::FixedClock;
use yss_harness_core::{KnowledgeQuery, KnowledgeService};
use yss_project::docs::{DocCommand, DocSnapshot};
use yss_project_identity::OperationId;
use yss_project_model::doc::DocEdit;

struct Fixture {
    application: ApplicationState,
    metadata: std::path::PathBuf,
    _directory: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let project = Arc::new(yss_project::ProjectState::new());
        let created = project
            .create_project_transaction(
                "Knowledge",
                &directory.path().join("project"),
                OperationId::new(),
            )
            .unwrap();
        project
            .activate_project_from_path(&created.metadata_path)
            .unwrap();
        let nodes = NodeComponents::builtins().unwrap();
        let candidate = crate::session::build_current_project_candidate(
            ApplicationSessionEpoch::INITIAL,
            project,
            [],
            &nodes,
        )
        .unwrap();
        let application = ApplicationState::new(Arc::new(ApplicationSessionSlot::new(nodes)));
        application.install_candidate(candidate).unwrap();
        Self {
            application,
            metadata: created.metadata_path,
            _directory: directory,
        }
    }

    fn project(&self) -> ProjectInstanceId {
        self.application
            .capture_session()
            .unwrap()
            .project_instance_id()
            .clone()
    }

    fn command(&self, command: DocCommand) -> Option<DocSnapshot> {
        self.application
            .apply_doc_command(self.project(), OperationId::new(), command)
            .unwrap()
            .snapshot
    }

    fn document(&self) -> DocSnapshot {
        let created = self
            .command(DocCommand::Create {
                name: "Methods".into(),
            })
            .unwrap();
        self.edit(created, "Original heteroskedasticity method")
    }

    fn edit(&self, snapshot: DocSnapshot, text: &str) -> DocSnapshot {
        let edited = self
            .command(DocCommand::Edit {
                path: snapshot.path,
                version: snapshot.version,
                edits: vec![DocEdit::SetMarkdown {
                    markdown: text.into(),
                }],
            })
            .unwrap();
        self.command(DocCommand::Save {
            path: edited.path,
            version: edited.version,
        })
        .unwrap()
    }

    fn query(&self, text: &str) -> KnowledgeQuery {
        KnowledgeQuery {
            text: text.into(),
            scopes: vec![],
            project: Some(self.application.harness_project_binding().unwrap()),
            limit: 5,
        }
    }
}

#[derive(Default)]
struct PausedIndex {
    built: tokio::sync::Notify,
    release: tokio::sync::Notify,
    pause: std::sync::atomic::AtomicBool,
}

impl KnowledgeIndexPort for PausedIndex {
    fn build(
        &self,
        chunks: Vec<KnowledgeIndexChunk>,
    ) -> KnowledgeIndexFuture<'_, Arc<dyn KnowledgeIndexReaderPort>> {
        Box::pin(async move {
            let reader = yss_harness_tantivy::TantivyKnowledgeIndex
                .build(chunks)
                .await?;
            if self.pause.swap(false, std::sync::atomic::Ordering::SeqCst) {
                self.built.notify_one();
                self.release.notified().await;
            }
            Ok(reader)
        })
    }
}

#[tokio::test]
async fn project_sources_reject_changed_or_deleted_documents_and_remove_snapshots() {
    let fixture = Fixture::new();
    let original = fixture.document();
    let store = Arc::new(
        yss_harness_sqlite::SqliteHarnessStore::connect(fixture._directory.path().join("app-data"))
            .await
            .unwrap(),
    );
    let sources = Arc::new(ProjectKnowledgeService::new(
        fixture.application.clone(),
        store.clone(),
        Arc::new(FixedClock::new(1_000)),
    ));
    sources
        .rebuild(&fixture.project(), original.path.as_str())
        .await
        .unwrap();
    assert_eq!(
        sources.list(&fixture.project()).await.unwrap()[0].status,
        ProjectKnowledgeStatus::Ready
    );
    let index = Arc::new(PausedIndex::default());
    index.pause.store(true, std::sync::atomic::Ordering::SeqCst);
    let knowledge = Arc::new(KnowledgeService::new(sources.clone(), index.clone()));
    let query = fixture.query("heteroskedasticity");
    let search = {
        let knowledge = knowledge.clone();
        let query = query.clone();
        tokio::spawn(async move { knowledge.search(query).await.unwrap() })
    };
    tokio::time::timeout(std::time::Duration::from_secs(10), index.built.notified())
        .await
        .unwrap();
    let changed = fixture.edit(original, "Revised heteroskedasticity diagnostics");
    index.release.notify_one();
    assert!(search.await.unwrap().is_empty());
    assert_eq!(
        sources.list(&fixture.project()).await.unwrap()[0].status,
        ProjectKnowledgeStatus::Changed
    );
    sources
        .rebuild(&fixture.project(), changed.path.as_str())
        .await
        .unwrap();
    let hits = knowledge.search(query.clone()).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert!(hits[0].excerpt.contains("Revised"));
    let citation = hits[0].citation.clone();
    assert!(
        knowledge
            .inspect(&citation, query.project.as_ref().unwrap())
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        sources
            .citation_resource(&citation)
            .await
            .unwrap()
            .unwrap()
            .id,
        changed.path.as_str()
    );
    sources
        .remove(&fixture.project(), &citation.source_id)
        .await
        .unwrap();
    assert!(sources.list(&fixture.project()).await.unwrap().is_empty());
    assert!(knowledge.search(query.clone()).await.unwrap().is_empty());
    assert!(
        knowledge
            .inspect(&citation, query.project.as_ref().unwrap())
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .load_active_snapshot(None)
            .await
            .unwrap()
            .unwrap()
            .documents
            .is_empty()
    );
    sources
        .rebuild(&fixture.project(), changed.path.as_str())
        .await
        .unwrap();
    fixture.command(DocCommand::Delete {
        path: changed.path,
        version: changed.version,
    });
    assert_eq!(
        sources.list(&fixture.project()).await.unwrap()[0].status,
        ProjectKnowledgeStatus::Unavailable
    );
    assert!(knowledge.search(query).await.unwrap().is_empty());
    assert!(sources.citation_resource(&citation).await.is_err());
    // Unsaved edits remain usable, but external edits/deletion cannot validate
    // a cached source even when Project retains dirty text for recovery.
    let external = fixture.document();
    let dirty = fixture
        .command(DocCommand::Edit {
            path: external.path.clone(),
            version: external.version,
            edits: vec![DocEdit::SetMarkdown {
                markdown: "Unsaved heteroskedasticity discussion".into(),
            }],
        })
        .unwrap();
    sources
        .rebuild(&fixture.project(), dirty.path.as_str())
        .await
        .unwrap();
    assert_eq!(
        knowledge
            .search(fixture.query("heteroskedasticity"))
            .await
            .unwrap()
            .len(),
        1
    );
    let file = fixture.metadata.parent().unwrap().join(dirty.path.as_str());
    std::fs::write(&file, "external replacement").unwrap();
    assert_eq!(
        sources.list(&fixture.project()).await.unwrap()[0].status,
        ProjectKnowledgeStatus::Changed
    );
    assert!(
        knowledge
            .search(fixture.query("heteroskedasticity"))
            .await
            .unwrap()
            .is_empty()
    );
    std::fs::remove_file(&file).unwrap();
    assert_eq!(
        sources.list(&fixture.project()).await.unwrap()[0].status,
        ProjectKnowledgeStatus::Unavailable
    );
    assert!(
        fixture
            .application
            .read_doc(fixture.project(), dirty.path.clone())
            .unwrap()
            .dirty
    );
    assert!(
        sources
            .rebuild(&fixture.project(), dirty.path.as_str())
            .await
            .is_err()
    );
    assert!(
        knowledge
            .search(fixture.query("heteroskedasticity"))
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn source_selection_survives_reopen_and_never_crosses_project_boundaries() {
    let fixture = Fixture::new();
    let document = fixture.document();
    let store = Arc::new(
        yss_harness_sqlite::SqliteHarnessStore::connect(fixture._directory.path().join("app-data"))
            .await
            .unwrap(),
    );
    let sources = Arc::new(ProjectKnowledgeService::new(
        fixture.application.clone(),
        store.clone(),
        Arc::new(FixedClock::new(1_000)),
    ));
    sources
        .rebuild(&fixture.project(), document.path.as_str())
        .await
        .unwrap();
    let knowledge = KnowledgeService::new(
        sources.clone(),
        Arc::new(yss_harness_tantivy::TantivyKnowledgeIndex),
    );
    let old_binding = fixture.application.harness_project_binding().unwrap();
    let citation = knowledge
        .search(fixture.query("heteroskedasticity"))
        .await
        .unwrap()
        .remove(0)
        .citation;
    fixture
        .application
        .load_project_for_application(fixture.metadata.to_str().unwrap())
        .unwrap();
    assert_ne!(
        old_binding,
        fixture.application.harness_project_binding().unwrap()
    );
    assert_eq!(
        knowledge
            .search(fixture.query("heteroskedasticity"))
            .await
            .unwrap()[0]
            .citation,
        citation
    );
    assert!(
        knowledge
            .inspect(&citation, &old_binding)
            .await
            .unwrap()
            .is_none()
    );
    let other = Fixture::new();
    fixture
        .application
        .load_project_for_application(other.metadata.to_str().unwrap())
        .unwrap();
    assert!(sources.list(&fixture.project()).await.unwrap().is_empty());
    assert!(
        knowledge
            .search(fixture.query("heteroskedasticity"))
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        sources
            .remove(&fixture.project(), &citation.source_id)
            .await
            .is_err()
    );
    assert!(
        sources
            .rebuild(old_binding.project_instance_id(), document.path.as_str())
            .await
            .is_err()
    );
    fixture
        .application
        .load_project_for_application(fixture.metadata.to_str().unwrap())
        .unwrap();
    assert_eq!(sources.list(&fixture.project()).await.unwrap().len(), 1);
    // Recreate both adapters, retaining only durable SQLite selections and Project content.
    let restored = Arc::new(ProjectKnowledgeService::new(
        fixture.application.clone(),
        Arc::new(
            yss_harness_sqlite::SqliteHarnessStore::connect(
                fixture._directory.path().join("app-data"),
            )
            .await
            .unwrap(),
        ),
        Arc::new(FixedClock::new(2_000)),
    ));
    let restored_knowledge = KnowledgeService::new(
        restored,
        Arc::new(yss_harness_tantivy::TantivyKnowledgeIndex),
    );
    assert_eq!(
        restored_knowledge
            .search(fixture.query("heteroskedasticity"))
            .await
            .unwrap()[0]
            .citation,
        citation
    );
}
