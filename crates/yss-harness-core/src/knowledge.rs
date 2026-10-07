use std::collections::BTreeSet;
use std::sync::{Arc, RwLock};

mod builtins;
mod index;
mod retrieval;
#[cfg(test)]
mod tests;
mod tools;

pub use builtins::install_builtin_statistical_knowledge;
use index::{IndexedDocument, PreparedIndex};
use retrieval::{chunk_id, passages};
use yss_harness_contract::{
    KnowledgeChunkId, KnowledgeCitation, KnowledgeDocumentId, KnowledgeDocumentRecord,
    KnowledgeIndexFailure, KnowledgeIndexPort, KnowledgeIndexQuery, KnowledgeSearchHit,
    KnowledgeSourceId, KnowledgeSourceRecord, KnowledgeSourceStatus, KnowledgeSourceStorePort,
    PersistenceFailure, ProjectSessionBinding, SensitivityClass, SourceHash, UnixMillis,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnowledgeQuery {
    pub text: String,
    pub scopes: Vec<String>,
    pub project: Option<ProjectSessionBinding>,
    pub limit: u16,
}

pub struct KnowledgeService {
    store: Arc<dyn KnowledgeSourceStorePort>,
    index: Arc<dyn KnowledgeIndexPort>,
    cached: RwLock<Option<Arc<PreparedIndex>>>,
}

impl KnowledgeService {
    pub fn new(
        store: Arc<dyn KnowledgeSourceStorePort>,
        index: Arc<dyn KnowledgeIndexPort>,
    ) -> Self {
        Self {
            store,
            index,
            cached: RwLock::new(None),
        }
    }

    pub async fn inspect(
        &self,
        citation: &KnowledgeCitation,
        project: &ProjectSessionBinding,
    ) -> Result<Option<String>, KnowledgeError> {
        let reference = yss_harness_contract::KnowledgePassageReference {
            document_id: citation.document_id.clone(),
            chunk_id: citation.chunk_id.clone(),
        };
        Ok(self
            .read_passage(&reference, project)
            .await?
            .filter(|(current, _)| current == citation)
            .map(|(_, text)| text))
    }

    async fn read_passage(
        &self,
        reference: &yss_harness_contract::KnowledgePassageReference,
        project: &ProjectSessionBinding,
    ) -> Result<Option<(KnowledgeCitation, String)>, KnowledgeError> {
        let Some((source, document)) = self
            .store
            .read_active_document(&reference.document_id)
            .await?
        else {
            return Ok(None);
        };
        let metadata = IndexedDocument::new(&source, &document);
        if !metadata.visible(Some(project))
            || source.status != KnowledgeSourceStatus::Active
            || document.source_id != source.id
            || document.id != reference.document_id
            || document.source_hash != source.source_hash
        {
            return Ok(None);
        }
        let Some(text) = cited_passage(&document, &reference.chunk_id)?.map(str::to_owned) else {
            return Ok(None);
        };
        Ok(Some((
            KnowledgeCitation {
                source_id: source.id,
                document_id: document.id,
                chunk_id: reference.chunk_id.clone(),
                title: document.title,
                version: source.version,
                source_hash: document.source_hash,
            },
            text,
        )))
    }

    pub async fn search(
        &self,
        query: KnowledgeQuery,
    ) -> Result<Vec<KnowledgeSearchHit>, KnowledgeError> {
        validate_query(&query)?;
        let snapshot = self.prepared_index(query.project.as_ref()).await?;
        let mut allowed = BTreeSet::new();
        for (id, metadata) in &snapshot.documents {
            if metadata.matches(&query)? {
                allowed.insert(id.clone());
            }
        }
        let limit = usize::from(query.limit);
        let mut hits = Vec::new();
        while !allowed.is_empty() && hits.len() < limit {
            let candidates = snapshot
                .reader
                .search(KnowledgeIndexQuery {
                    text: query.text.clone(),
                    documents: allowed.iter().cloned().collect(),
                    limit: limit - hits.len(),
                })
                .await?;
            if candidates.is_empty() {
                break;
            }
            let remaining = allowed.len();
            for candidate in candidates {
                if !allowed.remove(&candidate.document_id) {
                    continue;
                }
                if let Some(hit) = self.current_hit(&snapshot, &query, candidate).await? {
                    hits.push(hit);
                }
                if hits.len() == limit {
                    break;
                }
            }
            if allowed.len() == remaining {
                return Err(KnowledgeIndexFailure.into());
            }
            // Invalidated sources must not consume the result limit. Requery the
            // remaining documents without rebuilding the immutable index.
        }
        Ok(hits)
    }

    async fn current_hit(
        &self,
        snapshot: &PreparedIndex,
        query: &KnowledgeQuery,
        candidate: yss_harness_contract::KnowledgeIndexHit,
    ) -> Result<Option<KnowledgeSearchHit>, KnowledgeError> {
        // The index never authorizes a read. Recheck the current owner after
        // ranking, including writes/deletions which happened during indexing.
        let Some((source, document)) = self
            .store
            .read_active_document(&candidate.document_id)
            .await?
        else {
            return Ok(None);
        };
        let metadata = IndexedDocument::new(&source, &document);
        if snapshot.documents.get(&candidate.document_id) != Some(&metadata)
            || !metadata.matches(query)?
        {
            return Ok(None);
        }
        let Some(passage) = cited_passage(&document, &candidate.chunk_id)? else {
            return Ok(None);
        };
        if !candidate.score.is_finite() || !passage.contains(&candidate.excerpt) {
            return Err(KnowledgeIndexFailure.into());
        }
        Ok(Some(KnowledgeSearchHit {
            citation: KnowledgeCitation {
                source_id: source.id,
                document_id: document.id,
                chunk_id: candidate.chunk_id,
                title: document.title,
                version: source.version,
                source_hash: document.source_hash,
            },
            excerpt: candidate.excerpt,
            score: candidate.score,
        }))
    }
}

fn cited_passage<'a>(
    document: &'a KnowledgeDocumentRecord,
    id: &KnowledgeChunkId,
) -> Result<Option<&'a str>, KnowledgeError> {
    for passage in passages(&document.body) {
        if chunk_id(document.id.as_str(), &document.source_hash, passage)? == *id {
            return Ok(Some(passage.text));
        }
    }
    Ok(None)
}

fn validate_query(query: &KnowledgeQuery) -> Result<(), KnowledgeError> {
    yss_harness_contract::SearchKnowledgeRequest {
        query: query.text.clone(),
        scopes: query.scopes.clone(),
        limit: query.limit,
    }
    .validate()
    .map_err(|_| KnowledgeError::InvalidQuery)
}

fn visible_to_project(
    binding: Option<&ProjectSessionBinding>,
    requested: Option<&ProjectSessionBinding>,
) -> bool {
    binding.is_none() || binding == requested
}

fn sensitivity_visible(
    sensitivity: SensitivityClass,
    binding: Option<&ProjectSessionBinding>,
    requested: Option<&ProjectSessionBinding>,
) -> bool {
    sensitivity != SensitivityClass::Restricted || (binding.is_some() && binding == requested)
}

fn scope_matches(document_scopes: &[String], requested_scopes: &[String]) -> bool {
    requested_scopes.is_empty()
        || requested_scopes.iter().any(|requested| {
            document_scopes.iter().any(|scope| {
                scope == requested
                    || scope.starts_with(&format!("{requested}."))
                    || requested.starts_with(&format!("{scope}."))
            })
        })
}

#[derive(Debug, thiserror::Error)]
pub enum KnowledgeError {
    #[error("knowledge query is invalid")]
    InvalidQuery,
    #[error("knowledge source integrity check failed")]
    SourceIntegrity,
    #[error("knowledge persistence failed")]
    Persistence(#[from] PersistenceFailure),
    #[error("knowledge search failed")]
    Index(#[from] KnowledgeIndexFailure),
}
