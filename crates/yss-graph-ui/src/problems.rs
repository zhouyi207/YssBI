//! Diagnostics of every project graph, from the same projections Canvas and Details use.
pub(super) mod location;
mod row;

use crate::overview::ProjectOverview;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Icon,
    button::{Button, ButtonVariants},
    dock::{BasePanel, Panel, PanelEvent},
    scroll::Scrollbar,
};
use gpui_kit::{
    AnyElement, App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    ListAlignment, ListState, Pixels, Render, SharedString, Subscription, Window, div, list,
    prelude::*, px,
};
use std::{cell::Cell, collections::BTreeSet, rc::Rc, sync::Arc};
use yss_graph_document::GraphResourcePath;
use yss_graph_editor::projection::EditorProjectionModel;
use yss_ui::{appearance, text::t, tool_panel::ToolPanel};

#[derive(Clone)]
pub struct LocateProblem {
    projection: Arc<EditorProjectionModel>,
    diagnostic: usize,
    related: Option<usize>,
}

impl LocateProblem {
    pub fn projection(&self) -> &Arc<EditorProjectionModel> {
        &self.projection
    }

    pub fn location(&self) -> Option<&location::Location> {
        let diagnostic = self.projection.diagnostics.get(self.diagnostic)?;
        match self.related {
            Some(index) => diagnostic.related.get(index),
            None => Some(&diagnostic.location),
        }
    }
}

#[derive(Clone)]
enum ProblemRow {
    Graph {
        path: GraphResourcePath,
        name: SharedString,
        count: usize,
        collapsed: bool,
    },
    Unavailable(GraphResourcePath),
    Diagnostic {
        projection: Arc<EditorProjectionModel>,
        index: usize,
    },
}

pub struct ProblemsPanel {
    focus: FocusHandle,
    overview: Option<Entity<ProjectOverview>>,
    observation: Option<Subscription>,
    items: Rc<[ProblemRow]>,
    collapsed: BTreeSet<GraphResourcePath>,
    rows: ListState,
    estimated_width: Rc<Cell<Pixels>>,
    language: &'static str,
}

impl ProblemsPanel {
    pub const PANEL_NAME: &'static str = "problems";

    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            overview: None,
            observation: None,
            items: Rc::default(),
            collapsed: BTreeSet::new(),
            rows: ListState::new(0, ListAlignment::Top, px(160.)),
            estimated_width: Default::default(),
            language: yss_ui::text::locale(),
        }
    }

    pub fn set_overview(
        &mut self,
        overview: Option<Entity<ProjectOverview>>,
        cx: &mut Context<Self>,
    ) {
        self.observation = overview
            .as_ref()
            .map(|overview| cx.observe(overview, |view, _, cx| view.rebuild(cx)));
        self.overview = overview;
        self.collapsed.clear();
        self.rebuild(cx);
    }

    fn rebuild(&mut self, cx: &mut Context<Self>) {
        let mut items = Vec::new();
        if let Some(overview) = &self.overview {
            let overview = overview.read(cx);
            for path in overview.unavailable() {
                items.push(ProblemRow::Unavailable(path.clone()));
            }
            for graph in overview
                .graphs()
                .filter(|graph| !graph.projection.diagnostics.is_empty())
            {
                let collapsed = self.collapsed.contains(&graph.graph_path);
                items.push(ProblemRow::Graph {
                    path: graph.graph_path.clone(),
                    name: graph.name.clone().into(),
                    count: graph.projection.diagnostics.len(),
                    collapsed,
                });
                if !collapsed {
                    items.extend((0..graph.projection.diagnostics.len()).map(|index| {
                        ProblemRow::Diagnostic {
                            projection: graph.projection.clone(),
                            index,
                        }
                    }));
                }
            }
        }
        self.rows.reset(items.len());
        self.rows.clone().with_uniform_item_height(px(80.));
        self.items = items.into();
        cx.notify();
    }

    fn toggle_graph(&mut self, path: &GraphResourcePath, cx: &mut Context<Self>) {
        if !self.collapsed.remove(path) {
            self.collapsed.insert(path.clone());
        }
        self.rebuild(cx);
    }

    fn locate(
        &self,
        projection: &Arc<EditorProjectionModel>,
        diagnostic: usize,
        related: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        let current = self.overview.as_ref().is_some_and(|overview| {
            overview
                .read(cx)
                .graph(&projection.graph_path)
                .is_some_and(|graph| Arc::ptr_eq(&graph.projection, projection))
        });
        if current {
            cx.emit(LocateProblem {
                projection: projection.clone(),
                diagnostic,
                related,
            });
        }
    }

    fn render_item(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(item) = self.items.get(index) else {
            return div().into_any_element();
        };
        let muted = cx.theme().muted_foreground;
        match item {
            ProblemRow::Diagnostic { projection, index } => row::render(projection, *index, cx),
            ProblemRow::Unavailable(path) => div()
                .h_8()
                .px_2()
                .flex()
                .items_center()
                .gap_1p5()
                .text_xs()
                .child(
                    Icon::new(IconName::CircleX)
                        .size_3()
                        .text_color(cx.theme().danger),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .child(path.as_str().to_owned()),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .text_color(muted)
                        .child(t("panel.problemsUnreadable")),
                )
                .into_any_element(),
            ProblemRow::Graph {
                path,
                name,
                count,
                collapsed,
            } => {
                let path = path.clone();
                Button::new(SharedString::from(format!(
                    "problem-graph-{}",
                    path.as_str()
                )))
                .text()
                .w_full()
                .h_8()
                .px_2()
                .gap_1p5()
                .justify_start()
                .text_xs()
                .accessibility_label(name.clone())
                .toggled(!collapsed)
                .child(
                    Icon::new(if *collapsed {
                        IconName::ChevronRight
                    } else {
                        IconName::ChevronDown
                    })
                    .size_3()
                    .text_color(muted),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .child(name.clone()),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .text_color(muted)
                        .child(count.to_string()),
                )
                .child(div().flex_1())
                .on_click(cx.listener(move |view, _, _, cx| view.toggle_graph(&path, cx)))
                .into_any_element()
            }
        }
    }
}

impl Render for ProblemsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.items.len();
        let language = yss_ui::text::locale();
        if self.language != language {
            self.language = language;
            self.rows.remeasure();
        }
        let loaded = self
            .overview
            .as_ref()
            .is_some_and(|overview| overview.read(cx).loaded());
        let body = if !loaded || count == 0 {
            appearance::empty_state(
                IconName::TriangleAlert,
                t("panel.problems"),
                t(if loaded {
                    "panel.problemsEmpty"
                } else {
                    "panel.problemsChecking"
                }),
                cx,
            )
        } else {
            let owner = cx.entity().downgrade();
            list(self.rows.clone(), move |index, _, cx| {
                owner
                    .update(cx, |view, cx| view.render_item(index, cx))
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
            .child(ToolPanel::new(
                t("panel.problems"),
                div()
                    .relative()
                    .size_full()
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
                    .when(loaded && count > 0, |view| {
                        view.child(Scrollbar::vertical(&self.rows))
                    }),
            ))
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
        Self::PANEL_NAME
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
            .child(t("panel.problems"))
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
