//! Document routing and view-context installation; no parallel workbench topology.
use super::Workbench;
use crate::minds::{MindCanvas, MindEvent};
use gpui::{AppContext, Context, Window};
use gpui_component::dock::{DockPlacement, panel_handle};
use yss_project::minds::MindSnapshot;
use yss_project_model::mind::MindPath;

impl Workbench {
    pub(super) fn open_mind(
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
        if let Some(mind) = self.minds.get(&path).and_then(gpui::WeakEntity::upgrade) {
            self.present_panel(
                panel_handle(mind.clone()),
                DockPlacement::Center,
                window,
                cx,
            );
            mind.update(cx, |mind, cx| mind.focus_canvas(window, cx));
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
            let path = MindPath::parse(&query_path).map_err(anyhow::Error::msg)?;
            Ok(services.application.read_mind(identity, path)?)
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
                if intent
                    .as_deref()
                    .is_some_and(|id| !view.is_current_intent(id))
                {
                    return;
                }
                let applied = if let Some(snapshot) = result {
                    view.install_mind(snapshot, window, cx)
                        .update(cx, |mind, cx| mind.focus_canvas(window, cx));
                    true
                } else {
                    view.error = Some(crate::text::t("native.workbench.mindOpenFailed").into());
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

    pub(super) fn install_mind(
        &mut self,
        snapshot: MindSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Entity<MindCanvas> {
        let path = snapshot.path.as_str().to_owned();
        let services = self.services.clone();
        let mind = cx.new(|cx| MindCanvas::new(services, snapshot, cx));
        self.subscriptions
            .push(cx.subscribe(&mind, |view, mind, event, cx| {
                if matches!(event, MindEvent::Activated) {
                    view.details
                        .update(cx, |details, cx| details.set_mind(mind.downgrade(), cx));
                    view.activate_file(mind.read(cx).path().to_owned(), cx);
                }
                view.details.update(cx, |_, cx| cx.notify());
                cx.notify();
            }));
        self.minds.insert(path, mind.downgrade());
        self.present_panel(
            panel_handle(mind.clone()),
            DockPlacement::Center,
            window,
            cx,
        );
        mind
    }

    pub(super) fn refresh_minds(&self, window: &mut Window, cx: &mut Context<Self>) {
        for mind in self.minds.values().filter_map(gpui::WeakEntity::upgrade) {
            mind.update(cx, |mind, cx| mind.refresh(window, cx));
        }
    }
}
