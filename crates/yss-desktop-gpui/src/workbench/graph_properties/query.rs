use super::{GraphProperties, constants::ConstantOverview, signature::SignatureDraft};
use gpui::{Context, Window};
use yss_graph_document::GraphResourceKind;
use yss_project_history::FunctionDocument;

impl GraphProperties {
    pub(super) fn read(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(graph) = self.graph() else {
            return;
        };
        let graph = graph.read(cx);
        let project = graph.graph.project.clone();
        let path = graph.graph.projection.graph_path.clone();
        let version = graph.graph.editing.version;
        self.version = Some(version);
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.loading = true;
        self.ready = false;
        for field in &mut self.constants {
            field.value_loading = false;
            field.load_token = field.load_token.wrapping_add(1);
        }
        self.error = None;
        let task = self.services.run(move |services| {
            let application = &services.application;
            let document = application.current_graph_document(&project, &path, version)?;
            let constants = document
                .constants
                .values()
                .map(ConstantOverview::from_constant)
                .collect::<anyhow::Result<Vec<_>>>()?;
            let function = if path.kind() == GraphResourceKind::FunctionGraph {
                let snapshot = application.query_project_index(project.clone(), "zh-CN", false)?;
                let function = snapshot
                    .index
                    .function_graphs
                    .into_iter()
                    .find(|function| function.path == path.as_str())
                    .ok_or_else(|| anyhow::anyhow!("function was removed"))?;
                Some(FunctionDocument {
                    revision: function.function_revision,
                    signature: function.function_signature,
                })
            } else {
                None
            };
            application.current_graph_document(&project, &path, version)?;
            Ok((constants, function))
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.generation != generation {
                    return;
                }
                view.loading = false;
                if view
                    .graph()
                    .is_none_or(|graph| graph.read(cx).graph.editing.version != version)
                {
                    cx.notify();
                    return;
                }
                match result {
                    Ok((constants, function)) => {
                        view.install_constants(constants);
                        if view.signature.as_ref().map(|draft| &draft.baseline) != function.as_ref()
                        {
                            view.signature =
                                function.map(|function| SignatureDraft::new(function, window, cx));
                        }
                        view.ready = true;
                    }
                    Err(_) => view.error = Some("无法读取图属性，请刷新后重试。".into()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
