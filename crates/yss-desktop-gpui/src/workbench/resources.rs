//! Native resource creation consumes the existing application receipt and publication.
mod names;
mod operations;
use super::Workbench;
use gpui::{Context, Window};
pub(crate) use operations::GraphResourceAction;
use yss_graph_document::GraphResourceKind;
use yss_project_history::ResourceDocumentPatch;
use yss_project_identity::OperationId;

impl Workbench {
    pub(super) fn create_graph(
        &mut self,
        kind: GraphResourceKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || self.closing {
            return;
        }
        let Some(project) = &self.project else {
            return;
        };
        let identity = project.identity.clone();
        let expected = identity.clone();
        let lifecycle = self.lifecycle;
        self.busy = true;
        self.error = None;
        let publisher = self.services.clone();
        let task = self.services.run(move |services| {
            let operation = OperationId::new();
            let receipt =
                match kind {
                    GraphResourceKind::EventGraph => services.application.create_event_graph(
                        identity,
                        "新事件图".into(),
                        operation,
                    )?,
                    GraphResourceKind::FunctionGraph => services
                        .application
                        .create_function_graph(identity, "新函数图".into(), operation)?,
                };
            let path = receipt
                .deltas
                .iter()
                .find_map(|delta| match &delta.payload {
                    ResourceDocumentPatch::ResourceLifecycle(patch) if patch.before.is_none() => {
                        patch.after.as_ref().map(|after| after.path.to_string())
                    }
                    _ => None,
                });
            publisher.publish_resource(receipt);
            Ok(path)
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|value| value);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.lifecycle != lifecycle
                    || view
                        .project
                        .as_ref()
                        .is_none_or(|project| project.identity != expected)
                {
                    return;
                }
                view.busy = false;
                match result {
                    Ok(Some(path)) => view.open_graph(path, window, cx),
                    Ok(None) => {
                        view.error = Some("图已创建，请刷新项目目录后打开。".into());
                        view.refresh_project(window, cx);
                    }
                    Err(_) => view.error = Some("无法创建图，请检查项目状态后重试。".into()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
