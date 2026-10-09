//! The canonical diagnostic projection is shared with Canvas and Details.
pub(super) mod location;
mod navigation;
mod row;

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, IntoElement, ListAlignment, ListState,
    Pixels, Render, Window, div, list, prelude::*, px,
};
use gpui_component::{
    ActiveTheme, Icon,
    dock::{BasePanel, Panel, PanelEvent},
    scroll::Scrollbar,
};
use gpui_kit_assets::IconName;
use std::{cell::Cell, rc::Rc, sync::Arc};
use yss_graph_editor::projection::EditorProjectionModel;

#[derive(Clone)]
pub(super) struct LocateProblem {
    projection: Arc<EditorProjectionModel>,
    diagnostic: usize,
    related: Option<usize>,
}

impl LocateProblem {
    fn location(&self) -> Option<&location::Location> {
        let diagnostic = self.projection.diagnostics.get(self.diagnostic)?;
        match self.related {
            Some(index) => diagnostic.related.get(index),
            None => Some(&diagnostic.location),
        }
    }
}

pub struct ProblemsPanel {
    focus: FocusHandle,
    projection: Option<Arc<EditorProjectionModel>>,
    rows: ListState,
    estimated_width: Rc<Cell<Pixels>>,
    language: &'static str,
}

impl ProblemsPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            projection: None,
            rows: ListState::new(0, ListAlignment::Top, px(160.)),
            estimated_width: Default::default(),
            language: crate::text::locale(),
        }
    }

    pub fn set_projection(
        &mut self,
        projection: Arc<EditorProjectionModel>,
        cx: &mut Context<Self>,
    ) {
        if self
            .projection
            .as_ref()
            .is_some_and(|old| Arc::ptr_eq(old, &projection))
        {
            return;
        }
        if let Some(old) = &self.projection
            && old.graph_path == projection.graph_path
        {
            self.rows
                .splice(0..old.diagnostics.len(), projection.diagnostics.len());
        } else {
            self.rows.reset(projection.diagnostics.len());
        }
        self.rows.clone().with_uniform_item_height(px(80.));
        self.projection = Some(projection);
        cx.notify();
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        if self.projection.take().is_some() {
            self.rows.reset(0);
            cx.notify();
        }
    }

    fn locate(
        &self,
        projection: &Arc<EditorProjectionModel>,
        diagnostic: usize,
        related: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        if self
            .projection
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, projection))
        {
            cx.emit(LocateProblem {
                projection: projection.clone(),
                diagnostic,
                related,
            });
        }
    }
}

impl Render for ProblemsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self
            .projection
            .as_ref()
            .map_or(0, |projection| projection.diagnostics.len());
        let language = crate::text::locale();
        if self.language != language {
            self.language = language;
            self.rows.remeasure();
        }
        let body = if count == 0 {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .p_4()
                .text_color(cx.theme().muted_foreground)
                .child(crate::text::t(if self.projection.is_some() {
                    "panel.problemsEmpty"
                } else {
                    "panel.problemsNoGraph"
                }))
                .into_any_element()
        } else {
            let owner = cx.entity().downgrade();
            let projection = self.projection.as_ref().unwrap().clone();
            list(self.rows.clone(), move |index, _, cx| {
                owner
                    .update(cx, |_, cx| row::render(&projection, index, cx))
                    .unwrap_or_else(|_| div().into_any_element())
            })
            .size_full()
            .into_any_element()
        };
        let rows = self.rows.clone();
        let estimated_width = self.estimated_width.clone();
        div()
            .id("problems")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .text_xs()
            .child(
                div()
                    .h_8()
                    .px_2()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(div().flex_1().child(crate::text::t("panel.problems")))
                    .child(div().text_color(cx.theme().muted_foreground).child(
                        crate::text::format("panel.problemsCount", &[("count", count.to_string())]),
                    )),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(body)
                    .on_children_prepainted(move |_, window, _| {
                        let width = rows.viewport_bounds().size.width;
                        if count > 0 && estimated_width.replace(width) != width {
                            // GPUI clears unmeasured heights on resize. Seed after its layout;
                            // visible measurements replace these hints without building every row.
                            rows.clone().with_uniform_item_height(px(80.));
                            window.request_animation_frame();
                        }
                    })
                    .when(count > 0, |view| {
                        view.child(Scrollbar::vertical(&self.rows))
                    }),
            )
    }
}

impl EventEmitter<PanelEvent> for ProblemsPanel {}
impl EventEmitter<LocateProblem> for ProblemsPanel {}
impl Focusable for ProblemsPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl BasePanel for ProblemsPanel {
    fn panel_name(&self) -> &'static str {
        "problems"
    }
    fn closable(&self, _: &App) -> bool {
        false
    }
}
impl Panel for ProblemsPanel {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(Icon::new(IconName::TriangleAlert).size_3())
            .child(crate::text::t("panel.problems"))
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
