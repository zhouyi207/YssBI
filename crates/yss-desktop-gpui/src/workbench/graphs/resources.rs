//! Committed resource receipts rebind native panels; projections still come from Application.
use gpui::{Context, Window};
use yss_application::events::CommittedResourceMutation;
use yss_graph_document::GraphResourcePath;
use yss_project_history::{ResourceDocumentPatch, ResourceKey, ResourceLifecycleKind};
use yss_project_identity::ResourceRevision;

use super::super::Workbench;

impl Workbench {
    pub(in crate::workbench) fn accept_graph_resources(
        &mut self,
        mutation: &CommittedResourceMutation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .project
            .as_ref()
            .is_none_or(|project| project.identity != mutation.project_instance_id)
        {
            return;
        }
        for moved in &mutation.moves {
            if !is_graph(moved.kind) {
                continue;
            }
            // Reference graphs also have move patches; only the destination delta versions this move.
            let Some(delta) = mutation.deltas.iter().find(|delta| {
                graph_path(&delta.resource) == Some(moved.to.as_ref())
                    && matches!(&delta.payload, ResourceDocumentPatch::ResourceMove(patch)
                        if patch.from == moved.from && patch.to == moved.to)
            }) else {
                continue;
            };
            let Ok(path) = GraphResourcePath::new(moved.to.to_string()) else {
                continue;
            };
            self.move_graph_resource(
                &moved.from,
                path,
                delta.from_revision,
                delta.to_revision,
                window,
                cx,
            );
        }
        for delta in &mutation.deltas {
            if let ResourceDocumentPatch::ResourceLifecycle(patch) = &delta.payload
                && patch.after.is_none()
                && let Some(before) = &patch.before
                && is_graph(before.kind)
                && graph_path(&delta.resource) == Some(before.path.as_ref())
            {
                self.remove_graph_resource(&before.path, delta.from_revision, window, cx);
            }
        }
    }

    fn move_graph_resource(
        &mut self,
        from: &str,
        to: GraphResourcePath,
        before: ResourceRevision,
        after: ResourceRevision,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if from == to.as_str() || after <= before {
            return;
        }
        let canvas = self.graphs.get(from).and_then(gpui::WeakEntity::upgrade);
        let moved = if let Some(canvas) = canvas {
            // A delayed receipt must not displace an independently opened destination panel.
            if self
                .graphs
                .get(to.as_str())
                .and_then(gpui::WeakEntity::upgrade)
                .is_some()
                || !canvas.update(cx, |canvas, cx| {
                    canvas.move_resource(to.clone(), before, after, cx)
                })
            {
                return;
            }
            self.graphs.remove(from);
            self.graphs
                .insert(to.as_str().to_owned(), canvas.downgrade());
            true
        } else {
            self.indexed_graph_revision(from)
                .is_none_or(|revision| revision <= before)
        };
        if self
            .graph_openings
            .get(from)
            .is_some_and(|opening| opening.revision.is_none_or(|revision| revision <= before))
            && let Some(opening) = self.graph_openings.remove(from)
        {
            let super::Opening { node, intent, .. } = opening;
            self.start_graph_read(to.as_str().to_owned(), node, intent, window, cx);
        }
        if moved {
            if let Some(root) = &self.layout_root {
                self.services
                    .layouts
                    .remap_viewport(root, from, Some(to.as_str()));
            }
            self.persist_layout(cx);
        }
    }

    fn remove_graph_resource(
        &mut self,
        path: &str,
        before: ResourceRevision,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let canvas = self.graphs.get(path).and_then(gpui::WeakEntity::upgrade);
        let removed = if let Some(canvas) = canvas {
            if canvas.read(cx).resource_revision() > before {
                return;
            }
            self.graphs.remove(path);
            self.retire_graph(canvas, window, cx);
            true
        } else {
            self.indexed_graph_revision(path)
                .is_none_or(|revision| revision <= before)
        };
        if self
            .graph_openings
            .get(path)
            .is_some_and(|opening| opening.revision.is_none_or(|revision| revision <= before))
            && let Some(opening) = self.graph_openings.remove(path)
            && let Some(intent) = opening.intent
        {
            self.finish_intent(&intent, false, window, cx);
        }
        if removed {
            if let Some(root) = &self.layout_root {
                self.services.layouts.remap_viewport(root, path, None);
            }
            self.persist_layout(cx);
        }
    }

    pub(super) fn indexed_graph_revision(&self, path: &str) -> Option<ResourceRevision> {
        let index = &self.project.as_ref()?.index;
        index
            .event_graphs
            .iter()
            .find(|graph| graph.path == path)
            .map(|graph| graph.revision)
            .or_else(|| {
                index
                    .function_graphs
                    .iter()
                    .find(|graph| graph.path == path)
                    .map(|graph| graph.revision)
            })
    }

    fn retire_graph(
        &mut self,
        canvas: gpui::Entity<crate::canvas::GraphCanvas>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .details
            .read(cx)
            .graph()
            .is_some_and(|current| current.entity_id() == canvas.entity_id())
        {
            self.details.update(cx, |details, cx| details.clear(cx));
            self.problems.update(cx, |problems, cx| problems.clear(cx));
            self.output
                .update(cx, |output, cx| output.set_graph(None, cx));
            self.results.update(cx, |results, cx| {
                results.set_graph(None);
                cx.notify();
            });
        }
        self.dock
            .update(cx, |dock, cx| dock.remove_panel(canvas, window, cx));
    }
}

fn is_graph(kind: ResourceLifecycleKind) -> bool {
    matches!(
        kind,
        ResourceLifecycleKind::EventGraph | ResourceLifecycleKind::FunctionGraph
    )
}

fn graph_path(key: &ResourceKey) -> Option<&str> {
    match key {
        ResourceKey::Graph(path) => Some(path.as_str()),
        ResourceKey::Function(path) => Some(&path.0),
        _ => None,
    }
}
