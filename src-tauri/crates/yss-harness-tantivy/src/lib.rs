//! Rebuildable BM25 knowledge index. Sources and authorization belong to Harness Core.

#![forbid(unsafe_code)]

mod collector;
mod index;
mod tokenizer;

use std::sync::Arc;
use yss_harness_contract::{
    KnowledgeIndexChunk, KnowledgeIndexFailure, KnowledgeIndexFuture, KnowledgeIndexPort,
    KnowledgeIndexReaderPort,
};

pub struct TantivyKnowledgeIndex;

impl KnowledgeIndexPort for TantivyKnowledgeIndex {
    fn build(
        &self,
        chunks: Vec<KnowledgeIndexChunk>,
    ) -> KnowledgeIndexFuture<'_, Arc<dyn KnowledgeIndexReaderPort>> {
        Box::pin(async move {
            let index = tokio::task::spawn_blocking(move || index::KnowledgeIndex::build(chunks))
                .await
                .map_err(|_| KnowledgeIndexFailure)?
                .map_err(|_| KnowledgeIndexFailure)?;
            Ok(Arc::new(index) as Arc<dyn KnowledgeIndexReaderPort>)
        })
    }
}
