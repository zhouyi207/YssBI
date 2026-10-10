//! A restored tab whose resource is unavailable, without a fabricated document or result.
use crate::appearance;
use gpui::{App, Context, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window};
use gpui_component::dock::{BasePanel, Panel, PanelEvent, PanelState};
use gpui_kit_assets::IconName;

pub(super) struct MissingPanel {
    state: PanelState,
    focus: FocusHandle,
}

impl MissingPanel {
    pub fn new(state: PanelState, cx: &mut Context<Self>) -> Self {
        Self {
            state,
            focus: cx.focus_handle(),
        }
    }
}

impl Render for MissingPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        appearance::empty_state(
            IconName::File,
            if self.state.panel_name == "result" {
                crate::text::t("native.workbench.previousRunResults")
            } else {
                crate::text::t("native.workbench.resourceUnavailable")
            },
            if self.state.panel_name == "result" {
                crate::text::t("native.workbench.expiredResultsHint")
            } else {
                crate::text::t("native.workbench.missingResourceHint")
            },
            cx,
        )
    }
}

impl EventEmitter<PanelEvent> for MissingPanel {}
impl Focusable for MissingPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl BasePanel for MissingPanel {
    fn panel_name(&self) -> &'static str {
        "unavailable-resource"
    }
    fn dump(&self, _: &App) -> PanelState {
        self.state.clone()
    }
}

impl Panel for MissingPanel {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        if self.state.panel_name == "result" {
            crate::text::t("native.workbench.previousResults").to_owned()
        } else if let gpui_component::dock::PanelInfo::Panel(info) = &self.state.info {
            info.get("graphPath")
                .or_else(|| info.get("documentPath"))
                .or_else(|| info.get("mindPath"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or(crate::text::t("native.workbench.resourceUnavailable"))
                .to_owned()
        } else {
            crate::text::t("native.workbench.resourceUnavailable").into()
        }
    }
}
