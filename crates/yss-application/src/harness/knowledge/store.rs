//! Knowledge port adapter: persistence stores snapshots; Project authorizes their use.
use super::*;

impl KnowledgeSourceStorePort for ProjectKnowledgeService {
    fn list_active_sources(
        &self,
    ) -> PersistenceFuture<'_, Result<Vec<KnowledgeSourceRecord>, PersistenceFailure>> {
        self.store.list_active_sources()
    }

    fn replace_source<'a>(
        &'a self,
        source: &'a KnowledgeSourceRecord,
        documents: &'a [KnowledgeDocumentRecord],
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        self.store.replace_source(source, documents)
    }

    fn mark_source_deleted<'a>(
        &'a self,
        source_id: &'a KnowledgeSourceId,
        updated_at: UnixMillis,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        self.store.mark_source_deleted(source_id, updated_at)
    }

    fn load_active_snapshot(
        &self,
        known_generation: Option<u64>,
    ) -> PersistenceFuture<'_, Result<Option<KnowledgeSourceSnapshot>, PersistenceFailure>> {
        Box::pin(async move {
            let Some(mut snapshot) = self.store.load_active_snapshot(known_generation).await?
            else {
                return Ok(None);
            };
            let scope = self.scope(None).await.map_err(persistence_error)?;
            snapshot
                .documents
                .retain_mut(|(source, document)| bind_source(&scope, source, document));
            self.revalidate(&scope).map_err(persistence_error)?;
            Ok(Some(snapshot))
        })
    }

    fn read_active_document<'a>(
        &'a self,
        document_id: &'a KnowledgeDocumentId,
    ) -> PersistenceFuture<
        'a,
        Result<Option<(KnowledgeSourceRecord, KnowledgeDocumentRecord)>, PersistenceFailure>,
    > {
        Box::pin(async move {
            let Some((mut source, mut document)) =
                self.store.read_active_document(document_id).await?
            else {
                return Ok(None);
            };
            if source.origin.is_none() {
                return Ok(Some((source, document)));
            }
            let scope = self.scope(None).await.map_err(persistence_error)?;
            if !bind_source(&scope, &mut source, &mut document) {
                return Ok(None);
            }
            let checked_source = source.clone();
            let (scope, status) = tokio::task::spawn_blocking(move || {
                let status = source_status(&scope, &checked_source)?;
                Ok::<_, ProjectKnowledgeError>((scope, status))
            })
            .await
            .map_err(|_| unavailable())?
            .map_err(persistence_error)?;
            if status != ProjectKnowledgeStatus::Ready {
                return Ok(None);
            }
            // Source deletion/replacement during the Project read must also win.
            let current = self.store.read_active_document(document_id).await?;
            if current.as_ref().is_none_or(|(current, _)| {
                current.source_hash != source.source_hash || current.origin != source.origin
            }) {
                return Ok(None);
            }
            self.revalidate(&scope).map_err(persistence_error)?;
            Ok(Some((source, document)))
        })
    }
}

fn bind_source(
    scope: &Scope,
    source: &mut KnowledgeSourceRecord,
    document: &mut KnowledgeDocumentRecord,
) -> bool {
    let Some(origin) = &source.origin else {
        return true;
    };
    if origin.project_key != scope.key {
        return false;
    }
    source.project = Some(scope.binding.clone());
    document.project = Some(scope.binding.clone());
    true
}

fn persistence_error(error: ProjectKnowledgeError) -> PersistenceFailure {
    match error {
        ProjectKnowledgeError::Persistence(error) => error,
        _ => unavailable(),
    }
}
