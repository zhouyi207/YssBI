//! Charts are routed through the root DockArea and existing project index.
use super::Workbench;
use crate::charts::{ChartEditor, ChartEvent, query::ChartRead};
use gpui::{AppContext, Context, Window};
use gpui_component::dock::{DockPlacement, panel_handle};
use yss_chart_document::ChartResourcePath;

impl Workbench {
    pub(super) fn open_chart(
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
        if let Some(chart) = self.charts.get(&path).and_then(gpui::WeakEntity::upgrade) {
            self.present_panel(
                panel_handle(chart.clone()),
                DockPlacement::Center,
                window,
                cx,
            );
            chart.update(cx, |view, cx| view.focus_chart(window, cx));
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
        let project = project.identity.clone();
        let expected = project.clone();
        let lifecycle = self.lifecycle;
        let query_path = path.clone();
        let job = self.services.run(move |services| {
            crate::charts::query::read(services, project, ChartResourcePath::parse(&query_path)?)
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
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
                let applied = if let Some(read) = result {
                    view.install_chart(read, window, cx)
                        .update(cx, |view, cx| view.focus_chart(window, cx));
                    true
                } else {
                    view.error = Some("无法打开图表，请刷新项目后重试。".into());
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
    pub(super) fn install_chart(
        &mut self,
        read: ChartRead,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Entity<ChartEditor> {
        let path = read.path.as_str().to_owned();
        let services = self.services.clone();
        let chart = cx.new(|cx| ChartEditor::new(services, read, cx));
        self.subscriptions
            .push(cx.subscribe(&chart, |view, chart, event, cx| {
                if matches!(event, ChartEvent::Activated) {
                    view.details
                        .update(cx, |details, cx| details.set_chart(chart.downgrade(), cx));
                    view.activate_file(chart.read(cx).path.as_str().to_owned(), cx);
                }
                view.details.update(cx, |_, cx| cx.notify());
                cx.notify();
            }));
        self.charts.insert(path, chart.downgrade());
        self.present_panel(
            panel_handle(chart.clone()),
            DockPlacement::Center,
            window,
            cx,
        );
        chart.update(cx, |view, cx| view.refresh(window, cx));
        chart
    }
    pub(super) fn refresh_charts(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(project) = &self.project else {
            return;
        };
        for chart in self.charts.values().filter_map(gpui::WeakEntity::upgrade) {
            chart.update(cx, |chart, cx| {
                chart.replace_catalog(project.index.clone(), window, cx)
            });
        }
    }
}
