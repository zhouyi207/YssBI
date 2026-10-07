use yss_harness_contract::{SensitivityClass, SourceHash};

use super::*;

fn fixture(id: &str) -> (KnowledgeSourceRecord, KnowledgeDocumentRecord) {
    let source = KnowledgeSourceRecord {
        id: KnowledgeSourceId::try_new(id).unwrap(),
        title: id.into(),
        version: "1".into(),
        license: "test".into(),
        source_hash: SourceHash::try_new("source-hash").unwrap(),
        status: KnowledgeSourceStatus::Active,
        sensitivity: SensitivityClass::Public,
        project: None,
        origin: None,
        updated_at: UnixMillis::from_existing(1),
    };
    let document = KnowledgeDocumentRecord {
        id: KnowledgeDocumentId::try_new(id).unwrap(),
        source_id: source.id.clone(),
        title: id.into(),
        body: "original content".into(),
        scopes: vec![],
        tags: vec![],
        source_hash: source.source_hash.clone(),
        project: None,
        sensitivity: SensitivityClass::Public,
    };
    (source, document)
}

#[tokio::test]
async fn source_replacement_is_atomic_and_invalidates_snapshots() {
    let store = SqliteHarnessStore::connect_in_memory().await.unwrap();
    let (mut source, document) = fixture("methods");
    let (foreign, foreign_document) = fixture("foreign");
    store
        .replace_source(&source, std::slice::from_ref(&document))
        .await
        .unwrap();
    store
        .replace_source(&foreign, std::slice::from_ref(&foreign_document))
        .await
        .unwrap();
    let before = store.load_active_snapshot(None).await.unwrap().unwrap();
    assert!(
        store
            .load_active_snapshot(Some(before.generation))
            .await
            .unwrap()
            .is_none()
    );

    // A document ID collision occurs after the transaction updated the source
    // and removed its previous document set. All those changes must roll back.
    source.title = "new title".into();
    let mut collision = foreign_document.clone();
    collision.source_id = source.id.clone();
    let failure = store
        .replace_source(&source, &[document.clone(), collision])
        .await
        .unwrap_err();
    assert_eq!(failure.code, PersistenceFailureCode::Conflict);
    let after = store.load_active_snapshot(None).await.unwrap().unwrap();
    assert_eq!(before.documents, after.documents);
    assert_eq!(
        store
            .read_active_document(&foreign_document.id)
            .await
            .unwrap(),
        Some((foreign, foreign_document))
    );

    let mut replacement = document.clone();
    replacement.id = KnowledgeDocumentId::try_new("replacement").unwrap();
    replacement.body = "updated content".into();
    store
        .replace_source(&source, std::slice::from_ref(&replacement))
        .await
        .unwrap();
    let changed = store
        .load_active_snapshot(Some(after.generation))
        .await
        .unwrap()
        .unwrap();
    assert!(changed.generation > after.generation);
    assert!(
        store
            .read_active_document(&document.id)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store.read_active_document(&replacement.id).await.unwrap(),
        Some((source.clone(), replacement.clone()))
    );
    assert!(
        store
            .load_active_snapshot(Some(changed.generation))
            .await
            .unwrap()
            .is_none()
    );

    store
        .mark_source_deleted(&source.id, UnixMillis::from_existing(2))
        .await
        .unwrap();
    let deleted = store
        .load_active_snapshot(Some(changed.generation))
        .await
        .unwrap()
        .unwrap();
    assert!(
        deleted
            .documents
            .iter()
            .all(|(record, _)| record.id != source.id)
    );
    assert!(
        store
            .read_active_document(&replacement.id)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM knowledge_document WHERE source_id = ?")
            .bind(source.id.as_str())
            .fetch_one(&store.pool)
            .await
            .unwrap(),
        0
    );
}
