//! Neutral search projection. Source storage and access policy remain separate owners.

use std::{future::Future, pin::Pin, sync::Arc};

use crate::{KnowledgeChunkId, KnowledgeDocumentId};

pub type KnowledgeIndexFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, KnowledgeIndexFailure>> + Send + 'a>>;

pub struct KnowledgeIndexChunk {
    pub document_id: KnowledgeDocumentId,
    pub chunk_id: KnowledgeChunkId,
    pub title: String,
    pub body: String,
    pub scopes: Vec<String>,
    pub tags: Vec<String>,
}

pub struct KnowledgeIndexQuery {
    pub text: String,
    /// Core has already checked project, sensitivity, scope and source integrity.
    pub documents: Vec<KnowledgeDocumentId>,
    pub limit: usize,
}

pub struct KnowledgeIndexHit {
    pub document_id: KnowledgeDocumentId,
    pub chunk_id: KnowledgeChunkId,
    pub excerpt: String,
    pub score: f32,
}

#[derive(Debug, thiserror::Error)]
#[error("knowledge index unavailable")]
pub struct KnowledgeIndexFailure;

pub trait KnowledgeIndexPort: Send + Sync {
    /// Build an immutable, rebuildable projection outside domain state locks.
    fn build(
        &self,
        chunks: Vec<KnowledgeIndexChunk>,
    ) -> KnowledgeIndexFuture<'_, Arc<dyn KnowledgeIndexReaderPort>>;
}

pub trait KnowledgeIndexReaderPort: Send + Sync {
    /// At most one best passage per document, with access filtering before ranking.
    fn search(
        &self,
        query: KnowledgeIndexQuery,
    ) -> KnowledgeIndexFuture<'_, Vec<KnowledgeIndexHit>>;
}
