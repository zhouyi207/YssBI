//! Knowledge tool dispatch returns public content and a separate internal citation.

use serde_json::Value;
use yss_harness_contract::{
    CapabilityControl, CapabilityFailure, CapabilityFailureCode, KnowledgeMatch, KnowledgePassage,
    KnowledgePassageReference, KnowledgeSearchResult, READ_KNOWLEDGE_TOOL, ReadKnowledgeRequest,
    SEARCH_KNOWLEDGE_TOOL, SearchKnowledgeRequest,
};

use super::*;

pub(crate) enum KnowledgeRequest {
    Search(SearchKnowledgeRequest),
    Read(ReadKnowledgeRequest),
}

impl KnowledgeRequest {
    pub(crate) fn decode(tool: &str, request: &Value) -> Result<Self, CapabilityFailure> {
        let invalid = |_| {
            CapabilityFailure::new(CapabilityFailureCode::INVALID_REQUEST)
                .with_detail("capabilityId", tool)
        };
        match tool {
            SEARCH_KNOWLEDGE_TOOL => serde_json::from_value(request.clone())
                .map(Self::Search)
                .map_err(invalid),
            READ_KNOWLEDGE_TOOL => serde_json::from_value(request.clone())
                .map(Self::Read)
                .map_err(invalid),
            _ => Err(CapabilityFailure::new(
                CapabilityFailureCode::INVALID_REQUEST,
            )),
        }
    }

    pub(crate) fn validate(&self, tool: &str) -> Result<(), CapabilityFailure> {
        match self {
            Self::Search(request) => request
                .validate()
                .map_err(|error| error.into_failure(tool, "limit")),
            Self::Read(_) => Ok(()),
        }
    }
}

impl KnowledgeService {
    pub(crate) async fn invoke(
        &self,
        tool: &str,
        request: &Value,
        project: &ProjectSessionBinding,
        control: &CapabilityControl,
    ) -> Result<(Value, Option<KnowledgeCitation>), CapabilityFailure> {
        let request = KnowledgeRequest::decode(tool, request)?;
        request.validate(tool)?;
        control.check()?;
        tokio::select! {
            biased;
            _ = control.cancellation().cancelled() => {
                Err(control.check().err().unwrap_or_else(|| CapabilityFailure::new(CapabilityFailureCode::CANCELLED)))
            }
            _ = tokio::time::sleep_until(control.deadline().into()) => {
                Err(CapabilityFailure::new(CapabilityFailureCode::DEADLINE_ELAPSED))
            }
            result = self.dispatch(request, project) => result,
        }
    }

    async fn dispatch(
        &self,
        request: KnowledgeRequest,
        project: &ProjectSessionBinding,
    ) -> Result<(Value, Option<KnowledgeCitation>), CapabilityFailure> {
        let encode = |value: Result<Value, serde_json::Error>| {
            value.map_err(|_| CapabilityFailure::new(CapabilityFailureCode::INTERNAL_FAILURE))
        };
        match request {
            KnowledgeRequest::Search(request) => {
                let matches = self
                    .search(KnowledgeQuery {
                        text: request.query,
                        scopes: request.scopes,
                        project: Some(project.clone()),
                        limit: request.limit,
                    })
                    .await
                    .map_err(failure)?
                    .into_iter()
                    .map(|hit| KnowledgeMatch {
                        reference: KnowledgePassageReference {
                            document_id: hit.citation.document_id,
                            chunk_id: hit.citation.chunk_id,
                        },
                        source_id: hit.citation.source_id,
                        title: hit.citation.title,
                        excerpt: hit.excerpt,
                        score: hit.score,
                    })
                    .collect();
                Ok((
                    encode(serde_json::to_value(KnowledgeSearchResult { matches }))?,
                    None,
                ))
            }
            KnowledgeRequest::Read(request) => {
                let (citation, text) = self
                    .read_passage(&request.reference, project)
                    .await
                    .map_err(failure)?
                    .ok_or_else(|| {
                        CapabilityFailure::new(CapabilityFailureCode::RESULT_UNAVAILABLE)
                            .with_detail("reason", "knowledge_passage_unavailable")
                            .with_detail(
                                "nextStep",
                                "Search knowledge again for a current, accessible passage.",
                            )
                    })?;
                let passage = KnowledgePassage {
                    reference: request.reference,
                    source_id: citation.source_id.clone(),
                    title: citation.title.clone(),
                    text,
                };
                Ok((encode(serde_json::to_value(passage))?, Some(citation)))
            }
        }
    }
}

fn failure(error: KnowledgeError) -> CapabilityFailure {
    let (code, reason) = match error {
        KnowledgeError::InvalidQuery => (
            CapabilityFailureCode::INVALID_REQUEST,
            "invalid_knowledge_query",
        ),
        KnowledgeError::SourceIntegrity => (
            CapabilityFailureCode::RESULT_UNAVAILABLE,
            "knowledge_source_unavailable",
        ),
        KnowledgeError::Persistence(_) => (
            CapabilityFailureCode::RESULT_UNAVAILABLE,
            "knowledge_store_unavailable",
        ),
        KnowledgeError::Index(_) => (
            CapabilityFailureCode::RESULT_UNAVAILABLE,
            "knowledge_search_unavailable",
        ),
    };
    CapabilityFailure::new(code).with_detail("reason", reason)
}
