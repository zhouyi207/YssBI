//! A restored tab whose resource is unavailable, without a fabricated document or result.
use crate::appearance;
use gpui_kit::assets::IconName;
use gpui_kit::component::dock::{BasePanel, Panel, PanelEvent, PanelState};
use gpui_kit::{App, Context, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window};

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
            crate::text::t("native.workbench.resourceUnavailable"),
            crate::text::t("native.workbench.missingResourceHint"),
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
        if let gpui_kit::component::dock::PanelInfo::Panel(info) = &self.state.info {
            info.get("graphPath")
                .or_else(|| info.get("documentPath"))
                .or_else(|| info.get("mindPath"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    crate::text::t("native.workbench.resourceUnavailable").into_owned()
                })
        } else {
            crate::text::t("native.workbench.resourceUnavailable").into()
        }
    }
}
