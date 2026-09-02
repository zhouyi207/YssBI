use std::collections::BTreeSet;
use std::sync::Arc;

use yss_automation_contract::{
    KnowledgeChunkId, KnowledgeDocumentId, KnowledgeDocumentRecord, KnowledgeSearchHit,
    KnowledgeSourceId, KnowledgeSourceRecord, KnowledgeSourceStatus, KnowledgeSourceStorePort,
    PersistenceFailure, ProjectSessionBinding, SensitivityClass, SourceHash, UnixMillis,
};

const MAX_QUERY_BYTES: usize = 256;
const MAX_RESULTS: u16 = 20;
const MAX_EXCERPT_CHARS: usize = 480;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnowledgeQuery {
    pub text: String,
    pub scopes: Vec<String>,
    pub project: Option<ProjectSessionBinding>,
    pub limit: u16,
}

pub struct KnowledgeService {
    store: Arc<dyn KnowledgeSourceStorePort>,
}

pub async fn install_builtin_statistical_knowledge(
    store: Arc<dyn KnowledgeSourceStorePort>,
    now: UnixMillis,
) -> Result<(), KnowledgeError> {
    let documents = [
        (
            "dataset-quality-review",
            "Dataset quality review",
            "Before estimation, establish measurement scales, missingness, duplicate keys, outliers, and variable semantics. Stop when the dataset revision changes or required semantics remain unknown.",
            vec![
                "statistics.data_quality".to_owned(),
                "statistics.missingness".to_owned(),
            ],
            vec!["quality".to_owned(), "missingness".to_owned()],
        ),
        (
            "ols-diagnostics",
            "OLS assumptions and diagnostics",
            "OLS reporting should pair effect estimates and uncertainty with residual checks, influential-observation diagnostics, robustness checks, and explicit limitations.",
            vec![
                "statistics.regression.ols".to_owned(),
                "statistics.diagnostics".to_owned(),
            ],
            vec!["regression".to_owned(), "diagnostics".to_owned()],
        ),
    ];
    let digest = yss_canonical_hash::hash_canonical("yssbi.knowledge.builtin.v1", &documents)
        .map_err(|_| KnowledgeError::SourceIntegrity)?;
    let source_hash = SourceHash::try_new(
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
    )
    .map_err(|_| KnowledgeError::SourceIntegrity)?;
    let source_id = KnowledgeSourceId::try_new("yssbi-statistical-methods")
        .map_err(|_| KnowledgeError::SourceIntegrity)?;
    store
        .upsert_source(&KnowledgeSourceRecord {
            id: source_id.clone(),
            title: "YssBI Statistical Methods".to_owned(),
            version: "1.0.0".to_owned(),
            license: "YssBI project documentation".to_owned(),
            source_hash: source_hash.clone(),
            status: KnowledgeSourceStatus::Active,
            sensitivity: SensitivityClass::Public,
            project: None,
            updated_at: now,
        })
        .await?;
    for (id, title, body, scopes, tags) in documents {
        store
            .upsert_document(&KnowledgeDocumentRecord {
                id: KnowledgeDocumentId::try_new(id)
                    .map_err(|_| KnowledgeError::SourceIntegrity)?,
                source_id: source_id.clone(),
                title: title.to_owned(),
                body: body.to_owned(),
                scopes,
                tags,
                source_hash: source_hash.clone(),
                project: None,
                sensitivity: SensitivityClass::Public,
            })
            .await?;
    }
    Ok(())
}

impl KnowledgeService {
    pub fn new(store: Arc<dyn KnowledgeSourceStorePort>) -> Self {
        Self { store }
    }

    pub async fn search(
        &self,
        query: KnowledgeQuery,
    ) -> Result<Vec<KnowledgeSearchHit>, KnowledgeError> {
        validate_query(&query)?;
        let terms = tokenize(&query.text);
        let mut hits = Vec::new();
        for (source, document) in self.store.list_active_documents().await? {
            if source.status != KnowledgeSourceStatus::Active
                || source.id != document.source_id
                || source.source_hash != document.source_hash
            {
                return Err(KnowledgeError::SourceIntegrity);
            }
            if !visible_to_project(source.project.as_ref(), query.project.as_ref())
                || !visible_to_project(document.project.as_ref(), query.project.as_ref())
                || !sensitivity_visible(
                    source.sensitivity,
                    source.project.as_ref(),
                    query.project.as_ref(),
                )
                || !sensitivity_visible(
                    document.sensitivity,
                    document.project.as_ref(),
                    query.project.as_ref(),
                )
                || !scope_matches(&document.scopes, &query.scopes)
            {
                continue;
            }
            let score = score_document(
                &terms,
                &document.title,
                &document.scopes,
                &document.tags,
                &document.body,
            );
            if score == 0 {
                continue;
            }
            hits.push(KnowledgeSearchHit {
                citation: yss_automation_contract::KnowledgeCitation {
                    source_id: source.id,
                    document_id: document.id.clone(),
                    chunk_id: chunk_id(&document.id.to_string(), &document.source_hash)?,
                    title: document.title,
                    version: source.version,
                    source_hash: document.source_hash,
                },
                excerpt: excerpt(&document.body),
                score,
            });
        }
        hits.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.citation.title.cmp(&right.citation.title))
                .then_with(|| left.citation.document_id.cmp(&right.citation.document_id))
        });
        hits.truncate(usize::from(query.limit));
        Ok(hits)
    }
}

fn validate_query(query: &KnowledgeQuery) -> Result<(), KnowledgeError> {
    if query.text.trim().is_empty()
        || query.text.len() > MAX_QUERY_BYTES
        || query.limit == 0
        || query.limit > MAX_RESULTS
        || query.scopes.iter().any(|scope| scope.trim().is_empty())
    {
        return Err(KnowledgeError::InvalidQuery);
    }
    Ok(())
}

fn tokenize(text: &str) -> BTreeSet<String> {
    text.to_lowercase()
        .split(|character: char| !character.is_alphanumeric() && character != '_')
        .filter(|term| !term.is_empty())
        .map(str::to_owned)
        .collect()
}

fn score_document(
    terms: &BTreeSet<String>,
    title: &str,
    scopes: &[String],
    tags: &[String],
    body: &str,
) -> u32 {
    let title = title.to_lowercase();
    let scopes = scopes.join(" ").to_lowercase();
    let tags = tags.join(" ").to_lowercase();
    let body = body.to_lowercase();
    terms.iter().fold(0u32, |score, term| {
        score
            .saturating_add(u32::from(title.contains(term)) * 4)
            .saturating_add(u32::from(scopes.contains(term)) * 2)
            .saturating_add(u32::from(tags.contains(term)) * 2)
            .saturating_add(u32::from(body.contains(term)))
    })
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

fn chunk_id(
    document_id: &str,
    source_hash: &SourceHash,
) -> Result<KnowledgeChunkId, KnowledgeError> {
    let digest =
        yss_canonical_hash::hash_canonical("yssbi.knowledge.chunk.v1", &(document_id, source_hash))
            .map_err(|_| KnowledgeError::SourceIntegrity)?;
    let suffix = digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    KnowledgeChunkId::try_new(format!("chunk-{suffix}"))
        .map_err(|_| KnowledgeError::SourceIntegrity)
}

fn excerpt(body: &str) -> String {
    let mut value = body.chars().take(MAX_EXCERPT_CHARS).collect::<String>();
    if body.chars().count() > MAX_EXCERPT_CHARS {
        value.push('…');
    }
    value
}

#[derive(Debug, thiserror::Error)]
pub enum KnowledgeError {
    #[error("knowledge query is invalid")]
    InvalidQuery,
    #[error("knowledge source integrity check failed")]
    SourceIntegrity,
    #[error("knowledge persistence failed")]
    Persistence(#[from] PersistenceFailure),
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::test_support::InMemoryHarnessStore;
    use yss_automation_contract::{
        KnowledgeDocumentId, KnowledgeDocumentRecord, KnowledgeSourceId, KnowledgeSourceRecord,
        KnowledgeSourceStorePort, SensitivityClass, SourceHash, UnixMillis,
    };

    #[test]
    fn lexical_score_prioritizes_title_and_scope_over_body_only_match() {
        let terms = tokenize("regression diagnostics");
        let title_score = score_document(
            &terms,
            "Regression diagnostics",
            &["statistics.regression".to_owned()],
            &[],
            "details",
        );
        let body_score = score_document(&terms, "Notes", &[], &[], "regression diagnostics");

        assert!(title_score > body_score);
    }

    #[tokio::test]
    async fn deleted_sources_are_excluded_immediately_from_cited_results() {
        let store = Arc::new(InMemoryHarnessStore::default());
        let source_id = KnowledgeSourceId::try_new("methods").unwrap();
        let source_hash = SourceHash::try_new("hash-1").unwrap();
        store
            .upsert_source(&KnowledgeSourceRecord {
                id: source_id.clone(),
                title: "YssBI Methods".to_owned(),
                version: "1.0.0".to_owned(),
                license: "YssBI".to_owned(),
                source_hash: source_hash.clone(),
                status: KnowledgeSourceStatus::Active,
                sensitivity: SensitivityClass::Public,
                project: None,
                updated_at: UnixMillis::from_existing(1),
            })
            .await
            .unwrap();
        store
            .upsert_document(&KnowledgeDocumentRecord {
                id: KnowledgeDocumentId::try_new("ols-diagnostics").unwrap(),
                source_id: source_id.clone(),
                title: "OLS regression diagnostics".to_owned(),
                body: "Check residual assumptions and influential observations.".to_owned(),
                scopes: vec!["statistics.regression.ols".to_owned()],
                tags: vec!["diagnostics".to_owned()],
                source_hash,
                project: None,
                sensitivity: SensitivityClass::Public,
            })
            .await
            .unwrap();
        let service = KnowledgeService::new(store.clone());
        let query = KnowledgeQuery {
            text: "regression diagnostics".to_owned(),
            scopes: vec!["statistics.regression".to_owned()],
            project: None,
            limit: 5,
        };
        assert_eq!(service.search(query.clone()).await.unwrap().len(), 1);

        store
            .mark_source_deleted(&source_id, UnixMillis::from_existing(2))
            .await
            .unwrap();
        assert!(service.search(query).await.unwrap().is_empty());
    }
}
