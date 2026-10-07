//! A restored tab whose resource is unavailable, without a fabricated document or result.
use crate::appearance;
use gpui::{App, Context, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window};
use gpui_component::{
    IconName,
    dock::{BasePanel, Panel, PanelEvent, PanelState},
};

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
                "先前的运行结果"
            } else {
                "资源暂不可用"
            },
            if self.state.panel_name == "result" {
                "运行结果属于已结束的执行会话，请重新运行图后打开结果"
            } else {
                "资源可能已移动或删除，可从项目目录重新打开"
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
            "先前的结果".to_owned()
        } else if let gpui_component::dock::PanelInfo::Panel(info) = &self.state.info {
            info.get("graphPath")
                .or_else(|| info.get("documentPath"))
                .or_else(|| info.get("mindPath"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("资源暂不可用")
                .to_owned()
        } else {
            "资源暂不可用".into()
        }
    }
}
