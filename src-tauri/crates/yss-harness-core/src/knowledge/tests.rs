use std::sync::Arc;

use super::*;
use crate::test_support::InMemoryHarnessStore;
use yss_harness_contract::{
    KnowledgeDocumentId, KnowledgeDocumentRecord, KnowledgeSourceId, KnowledgeSourceRecord,
    KnowledgeSourceStorePort, SensitivityClass, SourceHash, UnixMillis,
};

#[tokio::test]
async fn bm25_prioritizes_title_and_returns_distinct_documents() {
    let store = Arc::new(InMemoryHarnessStore::default());
    for (id, title, body) in [
        (
            "title-match",
            "Regression diagnostics",
            "Model guidance".to_owned(),
        ),
        (
            "body-match",
            "Other notes",
            "Regression diagnostics".to_owned(),
        ),
        (
            "long-match",
            "Long notes",
            "Regression diagnostics. ".repeat(1500),
        ),
    ] {
        let (source, mut document) = fixture(id);
        document.title = title.into();
        document.body = body;
        store.replace_source(&source, &[document]).await.unwrap();
    }
    let service = service(store);
    let hits = service
        .search(KnowledgeQuery {
            text: "regression diagnostics".into(),
            scopes: vec![],
            project: None,
            limit: 3,
        })
        .await
        .unwrap();
    assert_eq!(hits.len(), 3);
    assert_eq!(hits[0].citation.document_id.as_str(), "title-match");
    assert!(
        hits.iter()
            .all(|hit| hit.score.is_finite() && hit.score > 0.0)
    );
    assert_eq!(
        hits.iter()
            .map(|hit| &hit.citation.document_id)
            .collect::<BTreeSet<_>>()
            .len(),
        3
    );
}

fn service(store: Arc<InMemoryHarnessStore>) -> KnowledgeService {
    KnowledgeService::new(store, Arc::new(yss_harness_tantivy::TantivyKnowledgeIndex))
}

#[tokio::test]
async fn deleted_sources_are_excluded_immediately_from_cited_results() {
    let store = Arc::new(InMemoryHarnessStore::default());
    let source_id = KnowledgeSourceId::try_new("methods").unwrap();
    let source_hash = SourceHash::try_new("hash-1").unwrap();
    let source = KnowledgeSourceRecord {
        id: source_id.clone(),
        title: "YssBI Methods".to_owned(),
        version: "1.0.0".to_owned(),
        license: "YssBI".to_owned(),
        source_hash: source_hash.clone(),
        status: KnowledgeSourceStatus::Active,
        sensitivity: SensitivityClass::Public,
        project: None,
        origin: None,
        updated_at: UnixMillis::from_existing(1),
    };
    let document = KnowledgeDocumentRecord {
        id: KnowledgeDocumentId::try_new("ols-diagnostics").unwrap(),
        source_id: source_id.clone(),
        title: "OLS regression diagnostics".to_owned(),
        body: "Check residual assumptions and influential observations.".to_owned(),
        scopes: vec!["statistics.regression.ols".to_owned()],
        tags: vec!["diagnostics".to_owned()],
        source_hash,
        project: None,
        sensitivity: SensitivityClass::Public,
    };
    store.replace_source(&source, &[document]).await.unwrap();
    let service = service(store.clone());
    let query = KnowledgeQuery {
        text: "regression diagnostics".to_owned(),
        scopes: vec!["statistics.regression".to_owned()],
        project: None,
        limit: 5,
    };
    let hits = service.search(query.clone()).await.unwrap();
    assert_eq!(hits.len(), 1);
    let project = ProjectSessionBinding::new(
        yss_project_identity::ProjectInstanceId::new(),
        yss_project_identity::ProjectSessionId::new("test-project-session"),
    );
    assert_eq!(
        service
            .inspect(&hits[0].citation, &project)
            .await
            .unwrap()
            .as_deref(),
        Some("Check residual assumptions and influential observations.")
    );
    let mut changed = hits[0].citation.clone();
    changed.version = "2.0.0".into();
    assert!(service.inspect(&changed, &project).await.unwrap().is_none());

    store
        .mark_source_deleted(&source_id, UnixMillis::from_existing(2))
        .await
        .unwrap();
    assert!(service.search(query).await.unwrap().is_empty());
    assert!(
        service
            .inspect(&hits[0].citation, &project)
            .await
            .unwrap()
            .is_none()
    );
}

fn fixture(id: &str) -> (KnowledgeSourceRecord, KnowledgeDocumentRecord) {
    let source = KnowledgeSourceRecord {
        id: KnowledgeSourceId::try_new(id).unwrap(),
        title: "Methods".into(),
        version: "1".into(),
        license: "Project notes".into(),
        source_hash: SourceHash::try_new("original").unwrap(),
        status: KnowledgeSourceStatus::Active,
        sensitivity: SensitivityClass::Public,
        project: None,
        origin: None,
        updated_at: UnixMillis::from_existing(1),
    };
    let document = KnowledgeDocumentRecord {
        id: KnowledgeDocumentId::try_new(id).unwrap(),
        source_id: source.id.clone(),
        title: "Methods".into(),
        body: "".into(),
        scopes: vec![],
        tags: vec![],
        source_hash: source.source_hash.clone(),
        project: None,
        sensitivity: SensitivityClass::Public,
    };
    (source, document)
}

struct ObservedIndex {
    builds: std::sync::atomic::AtomicUsize,
    gate: Option<(tokio::sync::Notify, tokio::sync::Notify)>,
}

impl KnowledgeIndexPort for ObservedIndex {
    fn build(
        &self,
        chunks: Vec<yss_harness_contract::KnowledgeIndexChunk>,
    ) -> yss_harness_contract::KnowledgeIndexFuture<
        '_,
        Arc<dyn yss_harness_contract::KnowledgeIndexReaderPort>,
    > {
        Box::pin(async move {
            self.builds
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let reader = yss_harness_tantivy::TantivyKnowledgeIndex
                .build(chunks)
                .await?;
            if let Some((built, release)) = &self.gate {
                built.notify_one();
                release.notified().await;
            }
            Ok(reader)
        })
    }
}

#[tokio::test]
async fn repeated_queries_reuse_index_until_knowledge_changes() {
    let store = Arc::new(InMemoryHarnessStore::default());
    let (source, mut document) = fixture("cache");
    document.body = "regression".into();
    store
        .replace_source(&source, std::slice::from_ref(&document))
        .await
        .unwrap();
    let index = Arc::new(ObservedIndex {
        builds: Default::default(),
        gate: None,
    });
    let service = KnowledgeService::new(store.clone(), index.clone());
    let query = KnowledgeQuery {
        text: "regression".into(),
        scopes: vec![],
        project: None,
        limit: 5,
    };
    assert_eq!(service.search(query.clone()).await.unwrap().len(), 1);
    assert_eq!(service.search(query.clone()).await.unwrap().len(), 1);
    assert_eq!(index.builds.load(std::sync::atomic::Ordering::Relaxed), 1);
    document.body = "changed content".into();
    store.replace_source(&source, &[document]).await.unwrap();
    assert!(service.search(query).await.unwrap().is_empty());
    assert_eq!(index.builds.load(std::sync::atomic::Ordering::Relaxed), 2);
}

#[tokio::test]
async fn source_deleted_while_index_builds_cannot_be_cited() {
    let store = Arc::new(InMemoryHarnessStore::default());
    let (source, mut document) = fixture("concurrent-delete");
    document.body = "regression".into();
    document.title = "regression".into();
    store.replace_source(&source, &[document]).await.unwrap();
    let (fallback_source, mut fallback) = fixture("fallback");
    fallback.body = "regression and additional background on analysis and inference".into();
    store
        .replace_source(&fallback_source, &[fallback])
        .await
        .unwrap();
    let index = Arc::new(ObservedIndex {
        builds: Default::default(),
        gate: Some((Default::default(), Default::default())),
    });
    let service = Arc::new(KnowledgeService::new(store.clone(), index.clone()));
    let task = tokio::spawn(async move {
        service
            .search(KnowledgeQuery {
                text: "regression".into(),
                scopes: vec![],
                project: None,
                limit: 1,
            })
            .await
    });
    let (built, release) = index.gate.as_ref().unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(10), built.notified())
        .await
        .unwrap();
    store
        .mark_source_deleted(&source.id, UnixMillis::from_existing(2))
        .await
        .unwrap();
    release.notify_one();
    let hits = task.await.unwrap().unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].citation.source_id, fallback_source.id);
}

#[tokio::test]
async fn chinese_queries_cite_the_matching_passage_and_reject_changed_content() {
    let store = Arc::new(InMemoryHarnessStore::default());
    let (source, mut document) = fixture("diagnostics");
    let evidence = "Breusch–Pagan (BP) 异方差检验用于检查回归残差的方差是否随解释变量变化。";
    document.body = format!(
        "{}\n{evidence}\n{}",
        "背景说明🧪".repeat(439),
        "附录🧪".repeat(1000)
    );
    store
        .replace_source(&source, std::slice::from_ref(&document))
        .await
        .unwrap();
    let service = service(store.clone());
    let query = KnowledgeQuery {
        text: "BP异方差检验该如何解释？".into(),
        scopes: vec![],
        project: None,
        limit: 5,
    };
    let hits = service.search(query.clone()).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert!(hits[0].excerpt.contains(evidence));
    assert!(hits[0].excerpt.chars().count() <= 482);
    let project = ProjectSessionBinding::new(
        yss_project_identity::ProjectInstanceId::new(),
        yss_project_identity::ProjectSessionId::new("session"),
    );
    let cited = service
        .inspect(&hits[0].citation, &project)
        .await
        .unwrap()
        .unwrap();
    assert!(cited.contains(evidence));
    assert!(cited.chars().count() <= 1_200);
    // Even an incorrectly retained source hash cannot make different text satisfy a citation.
    document.body = "Replaced document.".into();
    store
        .replace_source(&source, std::slice::from_ref(&document))
        .await
        .unwrap();
    assert!(
        service
            .inspect(&hits[0].citation, &project)
            .await
            .unwrap()
            .is_none()
    );
    assert!(service.search(query).await.unwrap().is_empty());

    install_builtin_statistical_knowledge(store, UnixMillis::from_existing(2))
        .await
        .unwrap();
    let chinese = service
        .search(KnowledgeQuery {
            text: "请检查回归残差诊断".into(),
            scopes: vec![],
            project: None,
            limit: 5,
        })
        .await
        .unwrap();
    assert!(
        chinese
            .iter()
            .any(|hit| hit.citation.document_id.as_str() == "ols-diagnostics")
    );
}

#[tokio::test]
async fn another_projects_inconsistent_source_cannot_break_current_knowledge_search() {
    let store = Arc::new(InMemoryHarnessStore::default());
    let (public_source, mut public_document) = fixture("public-methods");
    public_document.body = "Regression diagnostics".into();
    store
        .replace_source(&public_source, &[public_document])
        .await
        .unwrap();
    let (mut source, mut document) = fixture("private-methods");
    let project = ProjectSessionBinding::new(
        yss_project_identity::ProjectInstanceId::new(),
        yss_project_identity::ProjectSessionId::new("private-session"),
    );
    source.project = Some(project.clone());
    source.sensitivity = SensitivityClass::Restricted;
    document.project = Some(project.clone());
    document.sensitivity = SensitivityClass::Restricted;
    document.title = "Regression diagnostics".into();
    document.body = "Private regression diagnostics".into();
    document.source_hash = SourceHash::try_new("stale-index").unwrap();
    store
        .replace_source(&source, std::slice::from_ref(&document))
        .await
        .unwrap();
    let service = service(store);
    let query = KnowledgeQuery {
        text: "regression diagnostics".into(),
        scopes: vec![],
        project: None,
        limit: 1,
    };
    let hits = service.search(query.clone()).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].citation.document_id.as_str(), "public-methods");
    assert!(matches!(
        service
            .search(KnowledgeQuery {
                project: Some(project),
                ..query
            })
            .await,
        Err(KnowledgeError::SourceIntegrity)
    ));
}
