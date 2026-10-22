//! Runs of every project graph in this execution session, newest first. Logs keep the
//! process history; this panel shows run outcomes until their notices are dismissed.
mod failure;

use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, Sizable,
    button::{Button, ButtonVariants},
    dock::{BasePanel, Panel, PanelEvent},
    scroll::Scrollbar,
    spinner::Spinner,
};
use gpui_kit::{
    AnyElement, App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    ListAlignment, ListState, Render, SharedString, Subscription, Window, div, list, prelude::*,
    px,
};
use std::{collections::BTreeSet, rc::Rc};
use yss_application::graph::{editing::RunRecord, run::RunApplicationEventKind};
use yss_graph_execution::{plan::PlanSourceIdentity, run_registry::RunId};
use yss_graph_ui::overview::ProjectOverview;
use yss_ui::{
    appearance,
    tool_panel::{self, ToolPanel},
};

pub enum OutputEvent {
    Locate(PlanSourceIdentity),
}

pub struct OutputPanel {
    focus: FocusHandle,
    overview: Option<Entity<ProjectOverview>>,
    observation: Option<Subscription>,
    runs: Rc<[RunRecord]>,
    collapsed: BTreeSet<RunId>,
    rows: ListState,
}

impl OutputPanel {
    pub const PANEL_NAME: &'static str = "output";

    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            overview: None,
            observation: None,
            runs: Rc::default(),
            collapsed: BTreeSet::new(),
            rows: ListState::new(0, ListAlignment::Top, px(160.)),
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
        let runs = self
            .overview
            .as_ref()
            .map(|overview| {
                overview
                    .read(cx)
                    .runs()
                    .iter()
                    .rev()
                    .filter(|run| !run.acknowledged())
                    .cloned()
                    .collect::<Rc<[_]>>()
            })
            .unwrap_or_default();
        self.rows.reset(runs.len());
        self.runs = runs;
        cx.notify();
    }

    fn clear(&mut self, cx: &mut Context<Self>) {
        let finished = self
            .runs
            .iter()
            .filter(|run| run.finished_at().is_some())
            .map(|run| run.identity().clone())
            .collect::<Vec<_>>();
        if let Some(overview) = &self.overview {
            overview.update(cx, |overview, cx| overview.acknowledge(finished, cx));
        }
    }

    fn toggle(&mut self, run: RunId, cx: &mut Context<Self>) {
        if !self.collapsed.remove(&run) {
            self.collapsed.insert(run);
        }
        if let Some(index) = self
            .runs
            .iter()
            .position(|record| record.identity().run_id() == run)
        {
            self.rows.splice(index..index + 1, 1);
        }
        cx.notify();
    }

    fn locate(&self, run: RunId, source: PlanSourceIdentity, cx: &mut Context<Self>) {
        if self
            .runs
            .iter()
            .any(|record| record.identity().run_id() == run)
        {
            cx.emit(OutputEvent::Locate(source));
        }
    }

    fn render_run(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(record) = self.runs.get(index) else {
            return div().into_any_element();
        };
        let (muted, success, danger, info) = {
            let theme = cx.theme();
            (
                theme.muted_foreground,
                theme.success,
                theme.danger,
                theme.info,
            )
        };
        let identity = record.identity();
        let run = identity.run_id();
        let graph = self
            .overview
            .as_ref()
            .and_then(|overview| {
                overview
                    .read(cx)
                    .graph(identity.graph_path())
                    .map(|graph| graph.name.clone())
            })
            .unwrap_or_else(|| identity.graph_path().as_str().to_owned());
        let failure = match record.latest().kind() {
            RunApplicationEventKind::RunErrored { failure } => Some(failure),
            _ => None,
        };
        let (icon, status): (AnyElement, _) = match record.latest().kind() {
            RunApplicationEventKind::RunCompleted => (
                Icon::new(IconName::CircleCheck)
                    .size_3()
                    .text_color(success)
                    .into_any_element(),
                "native.canvas.runCompleted",
            ),
            RunApplicationEventKind::RunErrored { .. } => (
                Icon::new(IconName::CircleX)
                    .size_3()
                    .text_color(danger)
                    .into_any_element(),
                "native.canvas.runFailed",
            ),
            RunApplicationEventKind::RunCancelled => (
                Icon::new(IconName::CircleDashed)
                    .size_3()
                    .text_color(muted)
                    .into_any_element(),
                "plugins.taskStates.cancelled",
            ),
            _ if cx.reduce_motion() => (
                Icon::new(IconName::LoaderCircle)
                    .size_3()
                    .text_color(info)
                    .into_any_element(),
                "native.canvas.running",
            ),
            _ => (
                Spinner::new()
                    .xsmall()
                    .icon(IconName::LoaderCircle)
                    .color(info)
                    .into_any_element(),
                "native.canvas.running",
            ),
        };
        let started = chrono::DateTime::<chrono::Local>::from(record.started_at())
            .format("%H:%M:%S")
            .to_string();
        let duration = record.finished_at().and_then(|finished| {
            let seconds = finished
                .duration_since(record.started_at())
                .ok()?
                .as_secs_f64();
            // Short runs need hundredths to read as more than zero.
            let precision = if seconds < 10. { 2 } else { 1 };
            Some(crate::text::format(
                "panel.outputDuration",
                &[("value", format!("{seconds:.precision$}"))],
            ))
        });
        let expanded = failure.is_some() && !self.collapsed.contains(&run);
        let mut cells = vec![
            icon,
            div()
                .w(px(64.))
                .flex_shrink_0()
                .text_color(muted)
                .child(started)
                .into_any_element(),
            div()
                .min_w_0()
                .truncate()
                .child(graph.clone())
                .into_any_element(),
            div()
                .flex_shrink_0()
                .text_color(muted)
                .child(crate::text::t(status))
                .into_any_element(),
            div().flex_1().into_any_element(),
        ];
        if let Some(duration) = duration {
            cells.push(
                div()
                    .flex_shrink_0()
                    .text_color(muted)
                    .child(duration)
                    .into_any_element(),
            );
        }
        let font = cx.theme().mono_font_family.clone();
        // Only failures have details to expand; other runs are plain rows.
        let header = if failure.is_some() {
            cells.push(
                Icon::new(if expanded {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                })
                .size_3()
                .text_color(muted)
                .into_any_element(),
            );
            Button::new(SharedString::from(format!("output-run-{}", run.get())))
                .text()
                .w_full()
                .h_8()
                .px_3()
                .gap_2()
                .justify_start()
                .text_xs()
                .font_family(font)
                .accessibility_label(format!("{graph}: {}", crate::text::t(status)))
                .toggled(expanded)
                .children(cells)
                .on_click(cx.listener(move |view, _, _, cx| view.toggle(run, cx)))
                .into_any_element()
        } else {
            div()
                .w_full()
                .h_8()
                .px_3()
                .flex()
                .items_center()
                .gap_2()
                .text_xs()
                .font_family(font)
                .children(cells)
                .into_any_element()
        };
        div()
            .id(SharedString::from(format!("output-entry-{}", run.get())))
            .w_full()
            .flex()
            .flex_col()
            .child(header)
            .when_some(failure.filter(|_| expanded), |entry, failure| {
                let node = failure.source.as_ref().and_then(|source| {
                    let graph = self
                        .overview
                        .as_ref()?
                        .read(cx)
                        .graph(identity.graph_path())?;
                    let node = source.node()?.as_str();
                    graph
                        .projection
                        .nodes
                        .iter()
                        .find(|candidate| candidate.node_id.to_string() == node)
                        .map(|candidate| {
                            candidate
                                .display
                                .user_label
                                .as_deref()
                                .unwrap_or(&candidate.display.title)
                                .to_owned()
                        })
                });
                entry.child(failure::render(identity, failure, node, cx))
            })
            .into_any_element()
    }
}

impl Render for OutputPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let clearable = self.runs.iter().any(|run| run.finished_at().is_some());
        let body = if self.runs.is_empty() {
            appearance::empty_state(
                IconName::SquareTerminal,
                crate::text::t("panel.output"),
                crate::text::t("panel.outputEmpty"),
                cx,
            )
        } else {
            let owner = cx.entity().downgrade();
            div()
                .relative()
                .size_full()
                .child(
                    list(self.rows.clone(), move |index, _, cx| {
                        owner
                            .update(cx, |view, cx| view.render_run(index, cx))
                            .unwrap_or_else(|_| div().into_any_element())
                    })
                    .size_full(),
                )
                .child(Scrollbar::vertical(&self.rows))
                .into_any_element()
        };
        div()
            .id("output")
            .track_focus(&self.focus)
            .size_full()
            .child(
                ToolPanel::new(crate::text::t("panel.output"), body).tool(
                    tool_panel::button("clear-output-run-notices")
                        .icon(IconName::Trash)
                        .accessibility_label(crate::text::t("canvas.clearExecutionArtifacts"))
                        .tooltip(crate::text::t(if clearable {
                            "canvas.clearExecutionArtifacts"
                        } else {
                            "canvas.clearExecutionArtifactsDisabled"
                        }))
                        .disabled(!clearable)
                        .on_click(cx.listener(|view, _, _, cx| view.clear(cx))),
                ),
            )
    }
}

impl EventEmitter<PanelEvent> for OutputPanel {}
impl EventEmitter<OutputEvent> for OutputPanel {}
impl Focusable for OutputPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl BasePanel for OutputPanel {
    fn panel_name(&self) -> &'static str {
        Self::PANEL_NAME
    }
    fn closable(&self, _: &App) -> bool {
        false
    }
}

impl Panel for OutputPanel {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(Icon::new(IconName::SquareTerminal).size_3())
            .child(crate::text::t("panel.output"))
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
