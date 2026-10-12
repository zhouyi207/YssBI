//! Current graph output catalogue; result payload ownership remains in Application.
use crate::canvas::{GraphCanvas, ResultEntry};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    Icon, Sizable,
    button::{Button, ButtonVariants},
    dock::{BasePanel, Panel, PanelEvent},
};
use gpui_kit::{
    App, Context, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window, div,
    prelude::*, uniform_list,
};
use std::rc::Rc;
use yss_graph_execution::result::ResultReference;
use yss_ui::{appearance, tool_panel::ToolPanel};
pub struct ResultsPanel {
    focus: FocusHandle,
    entries: Rc<[ResultEntry]>,
}
pub enum ResultsEvent {
    Open(ResultReference),
}

impl ResultsPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            entries: Rc::default(),
        }
    }
    pub fn set_graph(&mut self, canvas: Option<&GraphCanvas>) {
        self.entries = canvas
            .map(|canvas| canvas.result_entries().clone())
            .unwrap_or_default();
    }
}
impl Render for ResultsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = if self.entries.is_empty() {
            appearance::empty_state(
                IconName::Inbox,
                yss_ui::text::t("panel.results"),
                yss_ui::text::t("native.workbench.resultsHint"),
                cx,
            )
        } else {
            uniform_list(
                "result-entries",
                self.entries.len(),
                cx.processor(|view, range: std::ops::Range<usize>, _, cx| {
                    range
                        .map(|index| {
                            let entry = &view.entries[index];
                            let reference = entry.reference;
                            div().h_8().flex().items_center().px_2().gap_2().child(
                                Button::new(("open-result", reference.result_id.get()))
                                    .small()
                                    .ghost()
                                    .icon(IconName::Table)
                                    .label(entry.title.clone())
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        if view
                                            .entries
                                            .iter()
                                            .any(|entry| entry.reference == reference)
                                        {
                                            cx.emit(ResultsEvent::Open(reference));
                                        }
                                    })),
                            )
                        })
                        .collect()
                }),
            )
            .size_full()
            .into_any_element()
        };
        div()
            .id("current-results")
            .track_focus(&self.focus)
            .size_full()
            .child(ToolPanel::new(yss_ui::text::t("panel.results"), body))
    }
}
impl EventEmitter<PanelEvent> for ResultsPanel {}
impl EventEmitter<ResultsEvent> for ResultsPanel {}
impl Focusable for ResultsPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl BasePanel for ResultsPanel {
    fn panel_name(&self) -> &'static str {
        "results"
    }
    fn closable(&self, _: &App) -> bool {
        false
    }
}

impl Panel for ResultsPanel {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(Icon::new(IconName::Table).size_3())
            .child(yss_ui::text::t("panel.results"))
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
