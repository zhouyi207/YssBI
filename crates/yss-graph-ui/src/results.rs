//! Output pins of every event graph, in the same computed state the canvas shows.
use crate::overview::ProjectOverview;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, Selectable, Sizable,
    button::{Button, ButtonVariants},
    dock::{BasePanel, Panel, PanelEvent},
    spinner::Spinner,
};
use gpui_kit::{
    AnyElement, App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render,
    SharedString, Subscription, Window, div, prelude::*, uniform_list,
};
use std::{collections::BTreeSet, rc::Rc};
use yss_graph_document::GraphResourcePath;
use yss_graph_execution::{
    plan::{PlanGraphId, PlanOutputRef, PlanPortAddress},
    result::{ResultCacheState, ResultReference},
};
use yss_node_protocol::PortDirection;
use yss_ui::{
    appearance,
    text::t,
    tool_panel::{self, ToolPanel},
};

pub struct ResultsPanel {
    focus: FocusHandle,
    overview: Option<Entity<ProjectOverview>>,
    observation: Option<Subscription>,
    rows: Rc<[ResultRow]>,
    collapsed: BTreeSet<GraphResourcePath>,
    show_all: bool,
}

pub enum ResultsEvent {
    Open(ResultReference),
    Compute {
        graph: GraphResourcePath,
        output: PlanOutputRef,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PinState {
    Computed(ResultReference),
    Computing,
    Uncomputed,
}

#[derive(Clone)]
enum ResultRow {
    Graph {
        path: GraphResourcePath,
        name: SharedString,
        computed: usize,
        total: usize,
        collapsed: bool,
    },
    Pin {
        graph: GraphResourcePath,
        output: PlanOutputRef,
        title: SharedString,
        state: PinState,
    },
}

impl ResultsPanel {
    pub const PANEL_NAME: &'static str = "results";

    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            overview: None,
            observation: None,
            rows: Rc::default(),
            collapsed: BTreeSet::new(),
            show_all: false,
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
        let mut rows = Vec::new();
        if let Some(overview) = &self.overview {
            let overview = overview.read(cx);
            for graph in overview.graphs() {
                let Some(results) = &graph.results else {
                    continue;
                };
                let running = overview
                    .running_outputs(&graph.graph_path)
                    .collect::<BTreeSet<_>>();
                let mut pins = graph
                    .projection
                    .nodes
                    .iter()
                    .flat_map(|node| {
                        let node_title = node
                            .display
                            .user_label
                            .as_deref()
                            .unwrap_or(&node.display.title);
                        node.ports
                            .iter()
                            .filter(|port| port.direction == PortDirection::Output && !port.orphan)
                            .map(move |port| {
                                let pin = port
                                    .display
                                    .instance_label
                                    .as_deref()
                                    .unwrap_or(&port.display.label);
                                (port, format!("{node_title} · {pin}"))
                            })
                    })
                    .map(|(port, title)| {
                        let output = PlanOutputRef::new(
                            PlanGraphId::from_existing(graph.graph_path.as_str().into()),
                            PlanPortAddress::from_existing(port.address.to_string().into()),
                        );
                        let state = if running.contains(&output) {
                            PinState::Computing
                        } else {
                            match results.outputs.get(&output) {
                                Some(ResultCacheState::Valid { result_id }) => {
                                    PinState::Computed(ResultReference {
                                        execution_session_id: results.execution_session_id,
                                        result_id: *result_id,
                                    })
                                }
                                _ => PinState::Uncomputed,
                            }
                        };
                        ResultRow::Pin {
                            graph: graph.graph_path.clone(),
                            output,
                            title: title.into(),
                            state,
                        }
                    })
                    .collect::<Vec<_>>();
                let total = pins.len();
                let computed = pins
                    .iter()
                    .filter(|row| {
                        matches!(
                            row,
                            ResultRow::Pin {
                                state: PinState::Computed(_),
                                ..
                            }
                        )
                    })
                    .count();
                if !self.show_all {
                    pins.retain(|row| {
                        !matches!(
                            row,
                            ResultRow::Pin {
                                state: PinState::Uncomputed,
                                ..
                            }
                        )
                    });
                }
                if pins.is_empty() && (!self.show_all || total == 0) {
                    continue;
                }
                let collapsed = self.collapsed.contains(&graph.graph_path);
                rows.push(ResultRow::Graph {
                    path: graph.graph_path.clone(),
                    name: graph.name.clone().into(),
                    computed,
                    total,
                    collapsed,
                });
                if !collapsed {
                    rows.extend(pins);
                }
            }
        }
        self.rows = rows.into();
        cx.notify();
    }

    fn toggle_graph(&mut self, path: &GraphResourcePath, cx: &mut Context<Self>) {
        if !self.collapsed.remove(path) {
            self.collapsed.insert(path.clone());
        }
        self.rebuild(cx);
    }

    fn activate(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(ResultRow::Pin {
            graph,
            output,
            state,
            ..
        }) = self.rows.get(index).cloned()
        else {
            return;
        };
        match state {
            PinState::Computed(reference) => cx.emit(ResultsEvent::Open(reference)),
            PinState::Uncomputed => cx.emit(ResultsEvent::Compute { graph, output }),
            PinState::Computing => {}
        }
    }

    fn render_row(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let (muted, success, info) = {
            let theme = cx.theme();
            (theme.muted_foreground, theme.success, theme.info)
        };
        match &self.rows[index] {
            ResultRow::Graph {
                path,
                name,
                computed,
                total,
                collapsed,
            } => {
                let path = path.clone();
                Button::new(SharedString::from(format!(
                    "result-graph-{}",
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
                        .child(format!("{computed}/{total}")),
                )
                .child(div().flex_1())
                .on_click(cx.listener(move |view, _, _, cx| view.toggle_graph(&path, cx)))
                .into_any_element()
            }
            ResultRow::Pin {
                graph,
                output,
                title,
                state,
            } => {
                let (icon, status): (AnyElement, _) = match state {
                    PinState::Computed(_) => (
                        Icon::new(IconName::CircleCheck)
                            .size_3()
                            .text_color(success)
                            .into_any_element(),
                        "panel.resultStates.computed",
                    ),
                    PinState::Computing => (
                        if cx.reduce_motion() {
                            Icon::new(IconName::LoaderCircle)
                                .size_3()
                                .text_color(info)
                                .into_any_element()
                        } else {
                            Spinner::new()
                                .xsmall()
                                .icon(IconName::LoaderCircle)
                                .color(info)
                                .into_any_element()
                        },
                        "panel.resultStates.computing",
                    ),
                    PinState::Uncomputed => (
                        Icon::new(IconName::CircleDashed)
                            .size_3()
                            .text_color(muted)
                            .into_any_element(),
                        "panel.resultStates.uncomputed",
                    ),
                };
                let action = match state {
                    PinState::Computed(_) => "panel.resultActions.open",
                    PinState::Computing => "panel.resultStates.computing",
                    PinState::Uncomputed => "panel.resultActions.compute",
                };
                Button::new(SharedString::from(format!(
                    "result-pin-{}#{}",
                    graph.as_str(),
                    output.port().as_str()
                )))
                .text()
                .w_full()
                .h_8()
                .pl_7()
                .pr_2()
                .gap_2()
                .justify_start()
                .text_xs()
                .disabled(*state == PinState::Computing)
                .accessibility_label(format!("{title}: {}", t(status)))
                .tooltip(t(action))
                .child(icon)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_left()
                        .child(title.clone()),
                )
                .child(div().flex_shrink_0().text_color(muted).child(t(status)))
                .on_click(cx.listener(move |view, _, _, cx| view.activate(index, cx)))
                .into_any_element()
            }
        }
    }
}

impl Render for ResultsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let loaded = self
            .overview
            .as_ref()
            .is_some_and(|overview| overview.read(cx).loaded());
        let body = if !loaded || self.rows.is_empty() {
            let detail = if !loaded {
                "panel.resultsLoading"
            } else if self.show_all {
                "panel.resultsNoGraphs"
            } else {
                "panel.resultsNoneComputed"
            };
            appearance::empty_state(IconName::Inbox, t("panel.results"), t(detail), cx)
        } else {
            uniform_list(
                "result-pins",
                self.rows.len(),
                cx.processor(|view, range: std::ops::Range<usize>, _, cx| {
                    range.map(|index| view.render_row(index, cx)).collect()
                }),
            )
            .size_full()
            .into_any_element()
        };
        let show_all = self.show_all;
        div()
            .id("project-results")
            .track_focus(&self.focus)
            .size_full()
            .child(
                ToolPanel::new(t("panel.results"), body).tool(
                    tool_panel::button("results-show-all")
                        .icon(IconName::ListFilter)
                        .selected(show_all)
                        .toggled(show_all)
                        .accessibility_label(t("panel.resultsShowAll"))
                        .tooltip(t(if show_all {
                            "panel.resultsShowComputed"
                        } else {
                            "panel.resultsShowAll"
                        }))
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.show_all = !view.show_all;
                            view.rebuild(cx);
                        })),
                ),
            )
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
        Self::PANEL_NAME
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
            .child(t("panel.results"))
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
