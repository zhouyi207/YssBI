//! Document routing and view-context installation; no parallel workbench topology.
use super::Workbench;
use crate::documents::{DocumentEditor, DocumentEvent};
use gpui::{AppContext, Context, Window};
use gpui_component::dock::{DockPlacement, panel_handle};
use yss_project::docs::DocSnapshot;
use yss_project_model::doc::DocPath;

impl Workbench {
    pub(super) fn open_document(
        &mut self,
        path: String,
        intent: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = &self.project else {
            return;
        };
        if self.busy || self.closing {
            return;
        }
        if let Some(document) = self
            .documents
            .get(&path)
            .and_then(gpui::WeakEntity::upgrade)
        {
            self.present_panel(
                panel_handle(document.clone()),
                DockPlacement::Center,
                window,
                cx,
            );
            document.update(cx, |document, cx| document.focus_editor(window, cx));
            if let Some(intent) = intent {
                self.finish_intent(&intent, true, window, cx);
            }
            return;
        }
        if !self.opening.insert(path.clone()) {
            if let Some(intent) = intent {
                self.finish_intent(&intent, false, window, cx);
            }
            return;
        }
        let identity = project.identity.clone();
        let expected = identity.clone();
        let lifecycle = self.lifecycle;
        let query_path = path.clone();
        let task = self.services.run(move |services| {
            let path = DocPath::parse(&query_path).map_err(anyhow::Error::msg)?;
            Ok(services.application.read_doc(identity, path)?)
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = task.await.ok().and_then(Result::ok);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.lifecycle != lifecycle
                    || view
                        .project
                        .as_ref()
                        .is_none_or(|project| project.identity != expected)
                {
                    return;
                }
                view.opening.remove(&path);
                let applied = if let Some(snapshot) = result {
                    view.install_document(snapshot, window, cx)
                        .update(cx, |document, cx| document.focus_editor(window, cx));
                    true
                } else {
                    view.error = Some("无法打开 Markdown 文档，请检查当前项目资源。".into());
                    false
                };
                if let Some(intent) = intent {
                    view.finish_intent(&intent, applied, window, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn install_document(
        &mut self,
        snapshot: DocSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Entity<DocumentEditor> {
        let path = snapshot.path.as_str().to_owned();
        let services = self.services.clone();
        let document = cx.new(|cx| DocumentEditor::new(services, snapshot, window, cx));
        self.subscriptions
            .push(cx.subscribe(&document, |view, document, event, cx| {
                if matches!(event, DocumentEvent::Activated) {
                    view.details.update(cx, |details, cx| {
                        details.set_document(document.downgrade(), cx)
                    });
                    view.activate_file(document.read(cx).path().to_owned(), cx);
                }
                view.details.update(cx, |_, cx| cx.notify());
                cx.notify();
            }));
        self.documents.insert(path, document.downgrade());
        self.present_panel(
            panel_handle(document.clone()),
            DockPlacement::Center,
            window,
            cx,
        );
        document
    }

    pub(super) fn refresh_documents(&self, window: &mut Window, cx: &mut Context<Self>) {
        for document in self
            .documents
            .values()
            .filter_map(gpui::WeakEntity::upgrade)
        {
            document.update(cx, |document, cx| document.refresh(window, cx));
        }
    }
}
