use std::collections::BTreeMap;

use yss_harness_contract::{
    KnowledgeIndexChunk, KnowledgeIndexReaderPort, KnowledgeSourceSnapshot,
};

use super::*;

pub(super) struct PreparedIndex {
    generation: u64,
    project: Option<ProjectSessionBinding>,
    pub documents: BTreeMap<KnowledgeDocumentId, IndexedDocument>,
    pub reader: Arc<dyn KnowledgeIndexReaderPort>,
}

#[derive(Eq, PartialEq)]
pub(super) struct IndexedDocument {
    source: KnowledgeSourceRecord,
    source_id: KnowledgeSourceId,
    source_hash: SourceHash,
    title: String,
    tags: Vec<String>,
    project: Option<ProjectSessionBinding>,
    sensitivity: SensitivityClass,
    scopes: Vec<String>,
}

impl IndexedDocument {
    pub fn new(source: &KnowledgeSourceRecord, document: &KnowledgeDocumentRecord) -> Self {
        Self {
            source: source.clone(),
            source_id: document.source_id.clone(),
            source_hash: document.source_hash.clone(),
            title: document.title.clone(),
            tags: document.tags.clone(),
            project: document.project.clone(),
            sensitivity: document.sensitivity,
            scopes: document.scopes.clone(),
        }
    }

    pub fn visible(&self, project: Option<&ProjectSessionBinding>) -> bool {
        visible_to_project(self.source.project.as_ref(), project)
            && visible_to_project(self.project.as_ref(), project)
            && sensitivity_visible(
                self.source.sensitivity,
                self.source.project.as_ref(),
                project,
            )
            && sensitivity_visible(self.sensitivity, self.project.as_ref(), project)
    }

    pub fn matches(&self, query: &KnowledgeQuery) -> Result<bool, KnowledgeError> {
        if !self.visible(query.project.as_ref()) || !scope_matches(&self.scopes, &query.scopes) {
            return Ok(false);
        }
        if self.source.status != KnowledgeSourceStatus::Active
            || self.source.id != self.source_id
            || self.source.source_hash != self.source_hash
        {
            return Err(KnowledgeError::SourceIntegrity);
        }
        Ok(true)
    }
}

impl KnowledgeService {
    pub(super) async fn prepared_index(
        &self,
        project: Option<&ProjectSessionBinding>,
    ) -> Result<Arc<PreparedIndex>, KnowledgeError> {
        let previous = self
            .cached
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
            .filter(|cached| cached.project.as_ref() == project)
            .cloned();
        let changed = self
            .store
            .load_active_snapshot(previous.as_ref().map(|cached| cached.generation))
            .await?;
        let Some(changed) = changed else {
            return previous.ok_or(KnowledgeError::SourceIntegrity);
        };
        let generation = changed.generation;
        let (documents, chunks) = tokio::task::spawn_blocking(move || prepare_documents(changed))
            .await
            .map_err(|_| KnowledgeIndexFailure)??;
        let reader = self.index.build(chunks).await?;
        let prepared = Arc::new(PreparedIndex {
            generation,
            project: project.cloned(),
            documents,
            reader,
        });
        // No source I/O or index construction holds this publication lock. A late
        // build cannot replace a newer snapshot published by a concurrent search.
        let mut cached = self
            .cached
            .write()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(current) = cached.as_ref().filter(|current| {
            current.project.as_ref() == project && current.generation >= generation
        }) {
            return Ok(current.clone());
        }
        *cached = Some(prepared.clone());
        Ok(prepared)
    }
}

fn prepare_documents(
    snapshot: KnowledgeSourceSnapshot,
) -> Result<
    (
        BTreeMap<KnowledgeDocumentId, IndexedDocument>,
        Vec<KnowledgeIndexChunk>,
    ),
    KnowledgeError,
> {
    let mut documents = BTreeMap::new();
    let mut chunks = Vec::new();
    for (source, document) in snapshot.documents {
        documents.insert(
            document.id.clone(),
            IndexedDocument::new(&source, &document),
        );
        for passage in passages(&document.body) {
            chunks.push(KnowledgeIndexChunk {
                document_id: document.id.clone(),
                chunk_id: chunk_id(document.id.as_str(), &document.source_hash, passage)?,
                title: document.title.clone(),
                body: passage.text.to_owned(),
                scopes: document.scopes.clone(),
                tags: document.tags.clone(),
            });
        }
    }
    Ok((documents, chunks))
}
