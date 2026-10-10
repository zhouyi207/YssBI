//! A graph tab before its first projection; it never invents an editable document.
use gpui_kit::component::{
    ActiveTheme, Sizable,
    button::Button,
    dock::{BasePanel, Panel, PanelEvent, PanelInfo, PanelState},
    spinner::Spinner,
};
use gpui_kit::{
    App, Context, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Task, Window, div,
    prelude::*,
};
use yss_graph_document::GraphResourcePath;
use yss_project_identity::ResourceRevision;

pub(super) enum OpeningEvent {
    Activated,
    Retry,
    Removed(Option<String>),
}

pub(in crate::workbench) struct Opening {
    pub(super) path: GraphResourcePath,
    pub(super) node: Option<String>,
    pub(super) intent: Option<String>,
    pub(super) revision: Option<ResourceRevision>,
    pub(super) task: Option<Task<()>>,
    pub(super) failed: bool,
    pub(super) removed: bool,
    focus: FocusHandle,
}

impl Opening {
    pub(in crate::workbench) fn clear_intent(&mut self) {
        if self.intent.take().is_some() {
            self.node = None;
        }
    }

    pub(super) fn new(
        path: GraphResourcePath,
        revision: Option<ResourceRevision>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            path,
            revision,
            node: None,
            intent: None,
            task: None,
            failed: false,
            removed: false,
            focus: cx.focus_handle(),
        }
    }
}

impl EventEmitter<OpeningEvent> for Opening {}
impl EventEmitter<PanelEvent> for Opening {}
impl Focusable for Opening {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl BasePanel for Opening {
    fn panel_name(&self) -> &'static str {
        "graph-editor"
    }
    fn set_active(&mut self, active: bool, _: &mut Window, cx: &mut Context<Self>) {
        if active {
            cx.emit(OpeningEvent::Activated);
        }
    }
    fn on_removed(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.removed = true;
        self.task = None;
        cx.emit(OpeningEvent::Removed(self.intent.take()));
    }
    fn dump(&self, _: &App) -> PanelState {
        let mut state = PanelState::new(self.panel_name());
        state.info = PanelInfo::Panel(serde_json::json!({ "graphPath": self.path.as_str() }));
        state
    }
}
impl Panel for Opening {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.path
            .as_str()
            .rsplit('/')
            .next()
            .unwrap_or(self.path.as_str())
            .trim_end_matches(".yssbi-event")
            .trim_end_matches(".yssbi-function")
            .to_owned()
    }
}
impl Render for Opening {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .p_6()
            .text_color(cx.theme().muted_foreground)
            .child(div().text_sm().child(self.path.as_str().to_owned()))
            .child(if self.failed {
                div()
                    .id("graph-open-error")
                    .role(gpui_kit::accesskit::Role::Alert)
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_3()
                    .child(crate::text::t("native.canvas.openFailed"))
                    .child(
                        Button::new("retry-open-graph")
                            .small()
                            .label(crate::text::t("common.retry"))
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(OpeningEvent::Retry))),
                    )
                    .into_any_element()
            } else {
                div()
                    .id("graph-opening")
                    .role(gpui_kit::accesskit::Role::Status)
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(Spinner::new().small())
                    .child(crate::text::t("native.canvas.opening"))
                    .into_any_element()
            })
    }
}
