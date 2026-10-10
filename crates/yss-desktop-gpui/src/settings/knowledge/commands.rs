use super::{Failure, KnowledgeSettings, ProjectScope};
use crate::settings::{Page, SettingsEvent, SettingsPanel};
use gpui_kit::Context;
use std::sync::Arc;
use yss_harness_contract::{KnowledgeSourceId, ProjectKnowledgeStatus};
use yss_project::ProjectIndex;
use yss_project_identity::ProjectInstanceId;
use yss_project_model::doc::DocPath;

pub(super) enum Mutation {
    Rebuild(DocPath),
    Remove(KnowledgeSourceId),
}

impl SettingsPanel {
    pub(crate) fn bind_knowledge_project(
        &mut self,
        project: Option<(ProjectInstanceId, Arc<ProjectIndex>)>,
        cx: &mut Context<Self>,
    ) {
        let same_project = self.knowledge.scope.as_ref().map(|scope| &scope.identity)
            == project.as_ref().map(|(identity, _)| identity);
        let documents_changed = match (&self.knowledge.scope, &project) {
            (Some(previous), Some((_, index))) if same_project => {
                !Arc::ptr_eq(&previous.index, index)
                    && !previous
                        .index
                        .docs
                        .iter()
                        .map(|doc| (&doc.path, &doc.name, doc.revision))
                        .eq(index
                            .docs
                            .iter()
                            .map(|doc| (&doc.path, &doc.name, doc.revision)))
            }
            _ => !same_project,
        };
        if !same_project {
            self.knowledge = KnowledgeSettings {
                generation: self.knowledge.generation.wrapping_add(1),
                ..KnowledgeSettings::default()
            };
        }
        self.knowledge.scope = project.map(|(identity, index)| ProjectScope { identity, index });
        if documents_changed {
            self.knowledge.selected = self
                .knowledge
                .selected
                .take()
                .filter(|selected| self.knowledge.can_add(selected));
            self.refresh_knowledge(cx);
            cx.notify();
        }
    }

    pub(crate) fn refresh_knowledge(&mut self, cx: &mut Context<Self>) {
        self.knowledge.refresh_pending = true;
        self.load_knowledge(cx);
    }

    pub(in crate::settings) fn load_knowledge(&mut self, cx: &mut Context<Self>) {
        if self.page != Page::Knowledge
            || !self.knowledge.refresh_pending
            || self.knowledge.loading
            || self.knowledge.pending
        {
            return;
        }
        let Some(scope) = &self.knowledge.scope else {
            return;
        };
        let project = scope.identity.clone();
        self.knowledge.generation = self.knowledge.generation.wrapping_add(1);
        let generation = self.knowledge.generation;
        self.knowledge.refresh_pending = false;
        self.knowledge.loading = true;
        if self.knowledge.failure == Some(Failure::Load) {
            self.knowledge.failure = None;
        }
        let service = self.services.application.harness.knowledge.clone();
        let expected = project.clone();
        let job = self
            .services
            .executor
            .spawn(async move { service.list(&project).await });
        // The retained settings entity, not its window, owns completion delivery.
        cx.spawn(async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            let _ = view.update(cx, |view, cx| {
                if !view.knowledge.current(&expected, generation) {
                    return;
                }
                view.knowledge.loading = false;
                if view.knowledge.refresh_pending {
                    view.load_knowledge(cx);
                } else if let Some(sources) = result {
                    view.knowledge.sources = sources;
                    view.knowledge.selected = view
                        .knowledge
                        .selected
                        .take()
                        .filter(|selected| view.knowledge.can_add(selected));
                } else {
                    view.knowledge.failure = Some(Failure::Load);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn knowledge_busy(&self) -> bool {
        self.busy() || self.knowledge.loading
    }

    pub(super) fn select_knowledge_document(
        &mut self,
        project: &ProjectInstanceId,
        generation: u64,
        path: DocPath,
        cx: &mut Context<Self>,
    ) {
        if self.page == Page::Knowledge
            && !self.knowledge_busy()
            && self.knowledge.current(project, generation)
            && self.knowledge.can_add(&path)
        {
            self.knowledge.selected = Some(path);
            cx.notify();
        }
    }

    pub(super) fn mutate_knowledge(
        &mut self,
        project: &ProjectInstanceId,
        generation: u64,
        mutation: Mutation,
        cx: &mut Context<Self>,
    ) {
        if self.page != Page::Knowledge
            || self.knowledge_busy()
            || !self.knowledge.current(project, generation)
        {
            return;
        }
        let valid = match &mutation {
            Mutation::Rebuild(path) => self
                .knowledge
                .scope
                .as_ref()
                .is_some_and(|scope| scope.index.docs.iter().any(|doc| &doc.path == path)),
            Mutation::Remove(id) => self
                .knowledge
                .sources
                .iter()
                .any(|source| &source.source_id == id),
        };
        if !valid {
            return;
        }
        self.knowledge.pending = true;
        self.knowledge.failure = None;
        let project = project.clone();
        let expected = project.clone();
        let service = self.services.application.harness.knowledge.clone();
        let job = self.services.executor.spawn(async move {
            match mutation {
                Mutation::Rebuild(path) => service.rebuild(&project, path.as_str()).await,
                Mutation::Remove(id) => service.remove(&project, &id).await,
            }
        });
        cx.spawn(async move |view, cx| {
            let success = job.await.is_ok_and(|result| result.is_ok());
            let _ = view.update(cx, |view, cx| {
                if !view.knowledge.current(&expected, generation) {
                    return;
                }
                view.knowledge.pending = false;
                view.knowledge.failure = (!success).then_some(Failure::Mutation);
                // Also re-read after failure: a completed store write may precede a stale-session error.
                view.refresh_knowledge(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn open_knowledge_document(
        &self,
        project: &ProjectInstanceId,
        generation: u64,
        source: &KnowledgeSourceId,
        cx: &mut Context<Self>,
    ) {
        if self.page != Page::Knowledge
            || self.knowledge_busy()
            || !self.knowledge.current(project, generation)
        {
            return;
        }
        if let Some(source) = self
            .knowledge
            .sources
            .iter()
            .find(|item| &item.source_id == source)
            && source.status != ProjectKnowledgeStatus::Unavailable
        {
            cx.emit(SettingsEvent::OpenDocument {
                project: project.clone(),
                path: source.path.clone(),
            });
        }
    }
}
