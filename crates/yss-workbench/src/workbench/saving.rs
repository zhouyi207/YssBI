//! Native save capture and receipt installation; each domain owner commits its own file.
use super::Workbench;
use crate::{
    canvas::{GraphCanvas, GraphCommandOutcome, GraphCommandRequest},
    charts::{ChartEditor, ChartSaveOutcome, ChartSaveRequest},
    databases::{DatabaseEditor, DatabaseSaveOutcome, DatabaseSaveRequest},
    documents::{DocumentEditor, DocumentSaveOutcome, DocumentSaveRequest},
    minds::{MindCanvas, MindSaveOutcome, MindSaveRequest},
    services::NativeServices,
    settings::{SettingsPanel, SettingsSaveOutcome, SettingsSaveRequest},
};
use gpui_kit::component::Root;
use gpui_kit::{App, Context, Entity, Window, WindowHandle};
use yss_ui::lifecycle::LogReleased;

pub(super) enum SaveTarget {
    Graph(Entity<GraphCanvas>),
    Document(Entity<DocumentEditor>),
    Mind(Entity<MindCanvas>),
    Database(Entity<DatabaseEditor>),
    Chart(Entity<ChartEditor>),
    Settings(Entity<SettingsPanel>, Option<WindowHandle<Root>>),
}
pub(super) enum SaveRequest {
    Graph(GraphCommandRequest),
    Document(DocumentSaveRequest),
    Mind(MindSaveRequest),
    Database(DatabaseSaveRequest),
    Chart(ChartSaveRequest),
    Settings(SettingsSaveRequest),
}
pub(super) enum SaveOutcome {
    Graph(Box<GraphCommandOutcome>),
    Document(DocumentSaveOutcome),
    Mind(MindSaveOutcome),
    Database(DatabaseSaveOutcome),
    Chart(ChartSaveOutcome),
    Settings(SettingsSaveOutcome),
}
impl SaveRequest {
    pub fn commit(self, owner: &NativeServices) -> SaveOutcome {
        match self {
            Self::Graph(request) => {
                SaveOutcome::Graph(Box::new(request.commit(&owner.application.application)))
            }
            Self::Document(request) => SaveOutcome::Document(request.commit(&owner.tasks)),
            Self::Mind(request) => SaveOutcome::Mind(request.commit(&owner.tasks)),
            Self::Database(request) => SaveOutcome::Database(request.commit(&owner.tasks)),
            Self::Chart(request) => SaveOutcome::Chart(request.commit(&owner.tasks)),
            Self::Settings(request) => SaveOutcome::Settings(request.commit(&owner.settings)),
        }
    }
}
impl SaveTarget {
    fn prepare(&self, cx: &mut Context<Workbench>) -> Option<SaveRequest> {
        match self {
            Self::Graph(view) => view
                .update(cx, |view, cx| view.prepare_save(cx))
                .map(SaveRequest::Graph),
            Self::Document(view) => view
                .update(cx, |view, cx| view.prepare_save(cx))
                .map(SaveRequest::Document),
            Self::Mind(view) => view
                .update(cx, |view, cx| view.prepare_save(cx))
                .map(SaveRequest::Mind),
            Self::Database(view) => view
                .update(cx, |view, cx| view.prepare_save(cx))
                .map(SaveRequest::Database),
            Self::Chart(view) => view
                .update(cx, |view, cx| view.prepare_save(cx))
                .map(SaveRequest::Chart),
            Self::Settings(view, _) => view
                .update(cx, |view, cx| view.prepare_save(cx))
                .map(SaveRequest::Settings),
        }
    }
    pub fn finish(
        &self,
        outcome: SaveOutcome,
        window: &mut Window,
        cx: &mut Context<Workbench>,
    ) -> bool {
        match (self, outcome) {
            (Self::Graph(view), SaveOutcome::Graph(outcome)) => {
                !view.update(cx, |view, cx| view.finish_command(*outcome, cx))
            }
            (Self::Document(view), SaveOutcome::Document(outcome)) => {
                let failed = outcome.failed();
                view.update(cx, |view, cx| view.finish_save(outcome, window, cx));
                failed
            }
            (Self::Mind(view), SaveOutcome::Mind(outcome)) => {
                let failed = outcome.failed();
                view.update(cx, |view, cx| view.finish_save(outcome, window, cx));
                failed
            }
            (Self::Database(view), SaveOutcome::Database(outcome)) => {
                let failed = outcome.is_failed();
                view.update(cx, |view, cx| view.finish_save(outcome, cx));
                failed
            }
            (Self::Chart(view), SaveOutcome::Chart(outcome)) => {
                let failed = outcome.failed();
                view.update(cx, |view, cx| view.finish_save(outcome, window, cx));
                failed
            }
            (Self::Settings(view, target_window), SaveOutcome::Settings(outcome)) => {
                let failed = outcome.is_err();
                finish_settings_save(view, *target_window, outcome, window, cx);
                failed
            }
            _ => unreachable!("save response belongs to its captured editor"),
        }
    }
    pub fn fail(&self, window: &mut Window, cx: &mut Context<Workbench>) {
        match self {
            Self::Settings(view, target_window) => finish_settings_save(
                view,
                *target_window,
                Err(yss_application::harness::models::ModelSettingsError::Unavailable),
                window,
                cx,
            ),
            Self::Chart(view) => view.update(cx, |view, cx| {
                view.finish_save(ChartSaveOutcome::failure(), window, cx)
            }),
            Self::Graph(view) => view.update(cx, |view, cx| view.fail_prepared_save(cx)),
            Self::Document(view) => view.update(cx, |view, cx| {
                view.finish_save(DocumentSaveOutcome::new(None, true), window, cx)
            }),
            Self::Mind(view) => view.update(cx, |view, cx| {
                view.finish_save(MindSaveOutcome::new(None, true), window, cx)
            }),
            Self::Database(view) => view.update(cx, |view, cx| {
                view.finish_save(DatabaseSaveOutcome::failed(), cx)
            }),
        }
    }
    fn abort(&self, window: &mut Window, cx: &mut Context<Workbench>) {
        match self {
            Self::Settings(view, _) => view.update(cx, |view, cx| view.cancel_prepared_save(cx)),
            Self::Chart(view) => view.update(cx, |view, cx| view.cancel_prepared_save(window, cx)),
            Self::Graph(view) => view.update(cx, |view, cx| view.cancel_prepared_save(cx)),
            Self::Document(view) => {
                view.update(cx, |view, cx| view.cancel_prepared_save(window, cx))
            }
            Self::Mind(view) => view.update(cx, |view, cx| view.cancel_prepared_save(window, cx)),
            Self::Database(view) => {
                view.update(cx, |view, cx| view.cancel_prepared_save(window, cx))
            }
        }
    }
}

fn finish_settings_save(
    panel: &Entity<SettingsPanel>,
    target_window: Option<WindowHandle<Root>>,
    outcome: SettingsSaveOutcome,
    fallback_window: &mut Window,
    cx: &mut App,
) {
    let mut outcome = Some(outcome);
    if let Some(handle) = target_window {
        handle
            .update(cx, |_, window, cx| {
                panel.update(cx, |view, cx| {
                    view.finish_save(outcome.take().unwrap(), window, cx);
                });
            })
            .log_released();
    }
    // A closed settings window still has a retained draft; an open one owns its controls.
    if let Some(outcome) = outcome {
        panel.update(cx, |view, cx| {
            view.finish_save(outcome, fallback_window, cx)
        });
    }
}

impl Workbench {
    pub(super) fn capture_saves(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<(Vec<SaveTarget>, Vec<SaveRequest>)> {
        let targets = self
            .graphs
            .values()
            .filter_map(gpui_kit::WeakEntity::upgrade)
            .filter(|view| view.read(cx).dirty(cx))
            .map(SaveTarget::Graph)
            .chain(
                self.documents
                    .values()
                    .filter_map(gpui_kit::WeakEntity::upgrade)
                    .filter(|view| view.read(cx).dirty())
                    .map(SaveTarget::Document),
            )
            .chain(
                self.minds
                    .values()
                    .filter_map(gpui_kit::WeakEntity::upgrade)
                    .filter(|view| view.read(cx).dirty())
                    .map(SaveTarget::Mind),
            )
            .chain(
                self.databases
                    .values()
                    .filter_map(gpui_kit::WeakEntity::upgrade)
                    .filter(|view| view.read(cx).dirty())
                    .map(SaveTarget::Database),
            )
            .chain(
                self.charts
                    .values()
                    .filter_map(gpui_kit::WeakEntity::upgrade)
                    .filter(|view| view.read(cx).dirty())
                    .map(SaveTarget::Chart),
            )
            .chain(
                self.settings
                    .read(cx)
                    .dirty()
                    .then(|| SaveTarget::Settings(self.settings.clone(), self.settings_window)),
            )
            .collect::<Vec<_>>();
        let mut requests = vec![];
        for target in &targets {
            let Some(request) = target.prepare(cx) else {
                for prepared in &targets[..requests.len()] {
                    prepared.abort(window, cx);
                }
                self.error = Some(crate::text::t("native.workbench.saveBlocked").into());
                cx.notify();
                return None;
            };
            requests.push(request);
        }
        Some((targets, requests))
    }
}
