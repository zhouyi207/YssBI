//! Project-bound read state; Application owns source selection and indexed content.
mod commands;
mod render;

use std::{collections::HashSet, sync::Arc};
use yss_harness_contract::ProjectKnowledgeSourceSummary;
use yss_project::{ProjectIndex, docs::DocIndexEntry};
use yss_project_identity::ProjectInstanceId;
use yss_project_model::doc::DocPath;

struct ProjectScope {
    identity: ProjectInstanceId,
    index: Arc<ProjectIndex>,
}

#[derive(Clone, Copy, PartialEq)]
enum Failure {
    Load,
    Mutation,
}

#[derive(Default)]
pub(super) struct KnowledgeSettings {
    scope: Option<ProjectScope>,
    sources: Vec<ProjectKnowledgeSourceSummary>,
    selected: Option<DocPath>,
    generation: u64,
    loading: bool,
    pub(super) pending: bool,
    refresh_pending: bool,
    failure: Option<Failure>,
}

impl KnowledgeSettings {
    fn can_add(&self, path: &DocPath) -> bool {
        self.scope
            .as_ref()
            .is_some_and(|scope| scope.index.docs.iter().any(|doc| &doc.path == path))
            && !self
                .sources
                .iter()
                .any(|source| source.path == path.as_str())
    }

    fn available_documents(&self) -> Vec<&DocIndexEntry> {
        let Some(scope) = &self.scope else {
            return vec![];
        };
        let indexed: HashSet<_> = self
            .sources
            .iter()
            .map(|source| source.path.as_str())
            .collect();
        let mut documents: Vec<_> = scope
            .index
            .docs
            .iter()
            .filter(|document| !indexed.contains(document.path.as_str()))
            .collect();
        documents.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.path.as_str().cmp(right.path.as_str()))
        });
        documents
    }

    fn current(&self, project: &ProjectInstanceId, generation: u64) -> bool {
        self.generation == generation
            && self
                .scope
                .as_ref()
                .is_some_and(|scope| &scope.identity == project)
    }
}
