//! GUI save submits the full captured draft to the existing chart writer.
use super::ChartEditor;
use crate::services::NativeServices;
use gpui_kit::{Context, Window};
use std::sync::Arc;
use yss_application::runtime::ApplicationServices;
use yss_chart_document::{ChartDocument, ChartResourcePath};
use yss_project_identity::{OperationId, ProjectInstanceId};

pub(crate) struct ChartSaveRequest {
    project: ProjectInstanceId,
    path: ChartResourcePath,
    document: ChartDocument,
}
pub(crate) struct ChartSaveOutcome {
    pub saved: Option<ChartDocument>,
    pub failed: bool,
}
impl ChartSaveRequest {
    pub fn commit(
        self,
        services: &ApplicationServices,
        owner: &Arc<NativeServices>,
    ) -> ChartSaveOutcome {
        match services.application.save_chart_resource(
            self.project,
            OperationId::new(),
            self.path,
            self.document.clone(),
            None,
        ) {
            Ok(mutation) => {
                owner.publish_resource(mutation);
                ChartSaveOutcome {
                    saved: Some(self.document),
                    failed: false,
                }
            }
            Err(_) => ChartSaveOutcome {
                saved: None,
                failed: true,
            },
        }
    }
}
impl ChartEditor {
    pub(crate) fn prepare_save(&mut self, cx: &mut Context<Self>) -> Option<ChartSaveRequest> {
        if self.busy() || !self.available {
            return None;
        }
        self.saving = true;
        self.error = None;
        self.changed(cx);
        Some(ChartSaveRequest {
            project: self.project.clone(),
            path: self.path.clone(),
            document: self.draft.clone(),
        })
    }
    pub(crate) fn finish_save(
        &mut self,
        outcome: ChartSaveOutcome,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.saving = false;
        if let Some(document) = outcome.saved {
            self.saved = document;
            self.external_change = false;
            self.refresh_again = false;
            // Save-all must accept receipts and start its project navigation before another read.
            cx.defer_in(window, |view, window, cx| view.refresh(window, cx));
        }
        if outcome.failed {
            self.error = Some(crate::text::t("native.charts.saveFailed").into());
        }
        cx.emit(gpui_kit::component::dock::PanelEvent::LayoutChanged);
        self.changed(cx);
    }
    pub(crate) fn cancel_prepared_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.saving = false;
        if self.refresh_again {
            self.refresh_again = false;
            self.refresh(window, cx);
        }
        self.changed(cx);
    }
    pub(crate) fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.dirty() {
            return;
        }
        let Some(request) = self.prepare_save(cx) else {
            return;
        };
        let owner = self.services.clone();
        let job = self
            .services
            .run(move |services| Ok(request.commit(services, &owner)));
        cx.spawn_in(window, async move |view, cx| {
            let outcome = job
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or(ChartSaveOutcome {
                    saved: None,
                    failed: true,
                });
            let _ = view.update_in(cx, |view, window, cx| view.finish_save(outcome, window, cx));
        })
        .detach();
    }
}
