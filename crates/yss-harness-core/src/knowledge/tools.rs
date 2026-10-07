//! Knowledge tool dispatch returns public content and a separate internal citation.

use yss_harness_contract::{
    AutomationCapabilityRequest as Request, AutomationCapabilityResult as ResultValue,
    CapabilityControl, CapabilityFailure, CapabilityFailureCode, KnowledgeMatch, KnowledgePassage,
    KnowledgePassageReference, KnowledgeSearchResult,
};

use super::*;

impl KnowledgeService {
    pub(crate) async fn invoke(
        &self,
        request: Request,
        project: &ProjectSessionBinding,
        control: &CapabilityControl,
    ) -> Result<(ResultValue, Option<KnowledgeCitation>), CapabilityFailure> {
        control.check()?;
        tokio::select! {
            biased;
            _ = control.cancellation().cancelled() => {
                Err(control.check().err().unwrap_or_else(|| CapabilityFailure::new(CapabilityFailureCode::Cancelled)))
            }
            _ = tokio::time::sleep_until(control.deadline().into()) => {
                Err(CapabilityFailure::new(CapabilityFailureCode::DeadlineElapsed))
            }
            result = self.dispatch(request, project) => result,
        }
    }

    async fn dispatch(
        &self,
        request: Request,
        project: &ProjectSessionBinding,
    ) -> Result<(ResultValue, Option<KnowledgeCitation>), CapabilityFailure> {
        match request {
            Request::SearchKnowledge(request) => {
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
                    ResultValue::KnowledgeSearch(KnowledgeSearchResult { matches }),
                    None,
                ))
            }
            Request::ReadKnowledge(request) => {
                let (citation, text) = self
                    .read_passage(&request.reference, project)
                    .await
                    .map_err(failure)?
                    .ok_or_else(|| {
                        CapabilityFailure::new(CapabilityFailureCode::ResultUnavailable)
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
                Ok((ResultValue::KnowledgePassage(passage), Some(citation)))
            }
            _ => Err(CapabilityFailure::new(
                CapabilityFailureCode::InvalidRequest,
            )),
        }
    }
}

fn failure(error: KnowledgeError) -> CapabilityFailure {
    let (code, reason) = match error {
        KnowledgeError::InvalidQuery => (
            CapabilityFailureCode::InvalidRequest,
            "invalid_knowledge_query",
        ),
        KnowledgeError::SourceIntegrity => (
            CapabilityFailureCode::ResultUnavailable,
            "knowledge_source_unavailable",
        ),
        KnowledgeError::Persistence(_) => (
            CapabilityFailureCode::ResultUnavailable,
            "knowledge_store_unavailable",
        ),
        KnowledgeError::Index(_) => (
            CapabilityFailureCode::ResultUnavailable,
            "knowledge_search_unavailable",
        ),
    };
    CapabilityFailure::new(code).with_detail("reason", reason)
}
