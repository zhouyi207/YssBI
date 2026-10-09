//! Read-only node documentation from the application's localized catalog.
use std::sync::Arc;

mod render;

use gpui::{AppContext, Context, Entity, Task};
use gpui_component::text::TextViewState;
use yss_node_protocol::NodeTypeId;
use yss_project_identity::ProjectInstanceId;

use crate::services::NativeServices;

#[derive(Clone, PartialEq, Eq)]
struct DocumentationTarget {
    project: ProjectInstanceId,
    node_type: NodeTypeId,
    language: &'static str,
}

enum DocumentationState {
    Missing,
    Loading,
    Ready(Entity<TextViewState>),
    Failed,
}

pub(super) struct NodeDocumentation {
    services: Arc<NativeServices>,
    target: Option<DocumentationTarget>,
    state: DocumentationState,
    expanded: bool,
    generation: u64,
    task: Option<Task<()>>,
}

impl NodeDocumentation {
    pub(super) fn new(services: Arc<NativeServices>) -> Self {
        Self {
            services,
            target: None,
            state: DocumentationState::Missing,
            expanded: true,
            generation: 0,
            task: None,
        }
    }

    fn set_target(&mut self, target: Option<DocumentationTarget>, cx: &mut Context<Self>) {
        if self.target == target {
            return;
        }
        if self
            .target
            .as_ref()
            .zip(target.as_ref())
            .is_none_or(|(old, next)| {
                old.project != next.project || old.node_type != next.node_type
            })
        {
            self.expanded = true;
        }
        self.task = None;
        self.target = target;
        self.state = DocumentationState::Missing;
        self.read(cx);
        cx.notify();
    }

    fn read(&mut self, cx: &mut Context<Self>) {
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let Some(target) = self.target.clone() else {
            return;
        };
        self.state = DocumentationState::Loading;
        let query = target.clone();
        let job = self.services.run(move |services| {
            Ok(services
                .application
                .node_documentation(&query.project, &query.node_type, query.language)?
                .filter(|markdown| !markdown.trim().is_empty()))
        });
        self.task = Some(cx.spawn(async move |view, cx| {
            let result = job
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update(cx, |view, cx| {
                if view.generation != generation
                    || view.target.as_ref() != Some(&target)
                    || target.language != crate::text::locale()
                {
                    return;
                }
                view.state = match result {
                    Ok(Some(markdown)) => DocumentationState::Ready(
                        cx.new(|cx| TextViewState::markdown(&markdown, cx).selectable(true)),
                    ),
                    Ok(None) => DocumentationState::Missing,
                    Err(_) => DocumentationState::Failed,
                };
                cx.notify();
            });
        }));
        cx.notify();
    }
}

impl super::DetailsPanel {
    pub(in crate::workbench) fn show_node_definition(
        &mut self,
        project: ProjectInstanceId,
        node_type: NodeTypeId,
        cx: &mut Context<Self>,
    ) {
        self.node_definition = Some(node_type.clone());
        self.connection_picker = None;
        self.log = None;
        self.error = None;
        self.epoch = self.epoch.wrapping_add(1);
        self.documentation.update(cx, |documentation, cx| {
            documentation.set_target(
                Some(DocumentationTarget {
                    project,
                    node_type,
                    language: crate::text::locale(),
                }),
                cx,
            );
        });
        cx.notify();
    }

    pub(in crate::workbench) fn show_node_properties(&mut self, cx: &mut Context<Self>) {
        self.node_definition = None;
        self.refresh_node_documentation(cx);
        cx.notify();
    }

    pub(super) fn refresh_node_documentation(&mut self, cx: &mut Context<Self>) {
        if self.node_definition.is_some() {
            return;
        }
        let target = self
            .node()
            .zip(self.graph())
            .map(|(node, graph)| DocumentationTarget {
                project: graph.read(cx).graph.project.clone(),
                node_type: node.node_type.clone(),
                language: crate::text::locale(),
            });
        self.documentation.update(cx, |documentation, cx| {
            documentation.set_target(target, cx);
        });
    }

    pub(super) fn clear_documentation(&mut self, cx: &mut Context<Self>) {
        self.node_definition = None;
        self.documentation.update(cx, |documentation, cx| {
            documentation.set_target(None, cx);
        });
    }
}
