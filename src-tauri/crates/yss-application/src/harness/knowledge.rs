//! Explicit project sources and current-document validation over the existing knowledge store.
use std::sync::Arc;

use crate::session::{ApplicationSession, ApplicationState};
use yss_harness_contract::*;
use yss_project_identity::ProjectInstanceId;
use yss_project_model::doc::DocPath;

mod store;
#[cfg(test)]
mod tests;

pub struct ProjectKnowledgeService {
    application: ApplicationState,
    store: Arc<dyn KnowledgeSourceStorePort>,
    clock: Arc<dyn ClockPort>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectKnowledgeError {
    #[error("knowledge project changed")]
    ProjectChanged,
    #[error("knowledge document is unavailable")]
    DocumentUnavailable,
    #[error("knowledge persistence failed")]
    Persistence(#[from] PersistenceFailure),
}

struct Scope {
    session: Arc<ApplicationSession>,
    binding: ProjectSessionBinding,
    key: String,
}

impl ProjectKnowledgeService {
    pub fn new(
        application: ApplicationState,
        store: Arc<dyn KnowledgeSourceStorePort>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            application,
            store,
            clock,
        }
    }

    async fn scope(
        &self,
        expected: Option<&ProjectInstanceId>,
    ) -> Result<Scope, ProjectKnowledgeError> {
        let application = self.application.clone();
        let expected = expected.cloned();
        tokio::task::spawn_blocking(move || {
            let (session, binding, key) = application
                .harness_conversation_scope()
                .map_err(|_| ProjectKnowledgeError::ProjectChanged)?;
            if expected
                .as_ref()
                .is_some_and(|expected| expected != binding.project_instance_id())
            {
                return Err(ProjectKnowledgeError::ProjectChanged);
            }
            Ok(Scope {
                session,
                binding,
                key,
            })
        })
        .await
        .map_err(|_| unavailable())?
    }

    fn revalidate(&self, scope: &Scope) -> Result<(), ProjectKnowledgeError> {
        self.application
            .revalidate_captured_session(&scope.session)
            .map_err(|_| ProjectKnowledgeError::ProjectChanged)
    }

    pub async fn list(
        &self,
        project: &ProjectInstanceId,
    ) -> Result<Vec<ProjectKnowledgeSourceSummary>, ProjectKnowledgeError> {
        let scope = self.scope(Some(project)).await?;
        let sources = self.store.list_active_sources().await?;
        let (scope, summaries) = tokio::task::spawn_blocking(move || {
            let mut summaries = Vec::new();
            for source in sources {
                let Some(origin) = source
                    .origin
                    .as_ref()
                    .filter(|origin| origin.project_key == scope.key)
                else {
                    continue;
                };
                let status = source_status(&scope, &source)?;
                summaries.push(ProjectKnowledgeSourceSummary {
                    source_id: source.id,
                    title: source.title,
                    path: origin.path.clone(),
                    status,
                    updated_at: source.updated_at,
                });
            }
            Ok::<_, ProjectKnowledgeError>((scope, summaries))
        })
        .await
        .map_err(|_| unavailable())??;
        self.revalidate(&scope)?;
        Ok(summaries)
    }

    /// Replace only this explicitly selected document. Project remains its content owner.
    pub async fn rebuild(
        &self,
        project: &ProjectInstanceId,
        path: &str,
    ) -> Result<(), ProjectKnowledgeError> {
        let scope = self.scope(Some(project)).await?;
        let path = DocPath::parse(path).map_err(|_| ProjectKnowledgeError::DocumentUnavailable)?;
        let updated_at = self.clock.now();
        let (scope, source, document) = tokio::task::spawn_blocking(move || {
            let document = scope
                .session
                .project()
                .read_doc_source(scope.binding.project_instance_id(), &path)
                .map_err(|_| ProjectKnowledgeError::DocumentUnavailable)?;
            let hash = content_hash(&document.content.0)?;
            let identity = digest(
                "yssbi.knowledge.project-document",
                &(&scope.key, path.as_str()),
            )?;
            let source_id = KnowledgeSourceId::try_new(format!("project-doc-{identity}"))
                .map_err(|_| unavailable())?;
            let source = KnowledgeSourceRecord {
                id: source_id.clone(),
                title: path.name().to_owned(),
                version: "project-document".into(),
                license: "Project-authored document".into(),
                source_hash: hash.clone(),
                status: KnowledgeSourceStatus::Active,
                sensitivity: SensitivityClass::Restricted,
                project: Some(scope.binding.clone()),
                origin: Some(ProjectKnowledgeOrigin {
                    project_key: scope.key.clone(),
                    path: path.as_str().to_owned(),
                }),
                updated_at,
            };
            let document = KnowledgeDocumentRecord {
                id: KnowledgeDocumentId::try_new(source_id.as_str()).map_err(|_| unavailable())?,
                source_id,
                title: source.title.clone(),
                body: document.content.0,
                scopes: vec![],
                tags: vec![path.as_str().to_owned()],
                source_hash: hash,
                project: Some(scope.binding.clone()),
                sensitivity: SensitivityClass::Restricted,
            };
            Ok::<_, ProjectKnowledgeError>((scope, source, document))
        })
        .await
        .map_err(|_| unavailable())??;
        self.revalidate(&scope)?;
        self.store.replace_source(&source, &[document]).await?;
        self.revalidate(&scope)
    }

    pub async fn remove(
        &self,
        project: &ProjectInstanceId,
        source_id: &KnowledgeSourceId,
    ) -> Result<(), ProjectKnowledgeError> {
        let scope = self.scope(Some(project)).await?;
        let sources = self.store.list_active_sources().await?;
        if !sources.iter().any(|source| {
            &source.id == source_id
                && source
                    .origin
                    .as_ref()
                    .is_some_and(|origin| origin.project_key == scope.key)
        }) {
            return Err(ProjectKnowledgeError::DocumentUnavailable);
        }
        self.revalidate(&scope)?;
        self.store
            .mark_source_deleted(source_id, self.clock.now())
            .await?;
        self.revalidate(&scope)
    }

    pub async fn citation_resource(
        &self,
        citation: &KnowledgeCitation,
    ) -> Result<Option<ProjectResourceRef>, ProjectKnowledgeError> {
        let Some((source, document)) = self.read_active_document(&citation.document_id).await?
        else {
            return Err(ProjectKnowledgeError::DocumentUnavailable);
        };
        if source.id != citation.source_id
            || source.source_hash != citation.source_hash
            || document.source_hash != citation.source_hash
        {
            return Err(ProjectKnowledgeError::DocumentUnavailable);
        }
        Ok(source.origin.map(|origin| ProjectResourceRef {
            kind: ProjectResourceKind::Doc,
            id: origin.path,
        }))
    }
}

fn source_status(
    scope: &Scope,
    source: &KnowledgeSourceRecord,
) -> Result<ProjectKnowledgeStatus, ProjectKnowledgeError> {
    let origin = source.origin.as_ref().ok_or_else(unavailable)?;
    let path = DocPath::parse(&origin.path).map_err(|_| unavailable())?;
    let document = match scope
        .session
        .project()
        .read_doc_source(scope.binding.project_instance_id(), &path)
    {
        Ok(document) => document,
        Err(yss_project::ProjectOperationError::ResourceRevisionConflict { .. }) => {
            return Ok(ProjectKnowledgeStatus::Changed);
        }
        Err(_) => return Ok(ProjectKnowledgeStatus::Unavailable),
    };
    if content_hash(&document.content.0)? != source.source_hash {
        Ok(ProjectKnowledgeStatus::Changed)
    } else if document.content.0.trim().is_empty() {
        Ok(ProjectKnowledgeStatus::Empty)
    } else {
        Ok(ProjectKnowledgeStatus::Ready)
    }
}

fn digest<T: serde::Serialize>(domain: &str, value: &T) -> Result<String, ProjectKnowledgeError> {
    let hash = yss_canonical_hash::hash_canonical(domain, value).map_err(|_| unavailable())?;
    Ok(hash.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn content_hash(text: &str) -> Result<SourceHash, ProjectKnowledgeError> {
    SourceHash::try_new(digest("yssbi.knowledge.project-content", &text)?)
        .map_err(|_| unavailable().into())
}

fn unavailable() -> PersistenceFailure {
    PersistenceFailure::new(PersistenceFailureCode::Unavailable)
}
