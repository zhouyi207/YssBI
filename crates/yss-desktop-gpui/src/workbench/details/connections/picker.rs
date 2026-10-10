//! The native List owns search, keyboard selection and virtualization.
use super::*;
use gpui_kit::component::{
    ActiveTheme,
    list::{ListDelegate, ListItem},
};
use gpui_kit::{App, Task, WeakEntity};
use yss_graph_editor::projection::{ConnectionCandidates, ConnectionDecision};

pub(super) struct ConnectionOption {
    address: PortAddress,
    label: String,
    replace: bool,
}

pub(super) fn options(
    candidates: ConnectionCandidates,
    projection: &EditorProjectionModel,
) -> Vec<ConnectionOption> {
    let wanted = candidates
        .candidates
        .iter()
        .filter(|candidate| !matches!(candidate.decision, ConnectionDecision::Invalid { .. }))
        .map(|candidate| &candidate.port)
        .collect();
    let labels = port_labels(projection, &wanted);
    candidates
        .candidates
        .into_iter()
        .filter_map(|candidate| {
            let replace = match candidate.decision {
                ConnectionDecision::Append => false,
                ConnectionDecision::Replace { .. } => true,
                ConnectionDecision::Invalid { .. } => return None,
            };
            Some(ConnectionOption {
                label: labels.get(&candidate.port)?.clone(),
                address: candidate.port,
                replace,
            })
        })
        .collect()
}

pub(in crate::workbench::details) struct ConnectionPicker {
    owner: WeakEntity<DetailsPanel>,
    source: PortAddress,
    direction: PortDirection,
    epoch: u64,
    options: Vec<ConnectionOption>,
    query: String,
    matches: Vec<usize>,
    selected: Option<IndexPath>,
    pub(super) loading: bool,
    pub(super) failed: bool,
}

impl ConnectionPicker {
    pub(super) fn new(
        owner: WeakEntity<DetailsPanel>,
        source: PortAddress,
        direction: PortDirection,
        epoch: u64,
    ) -> Self {
        Self {
            owner,
            source,
            direction,
            epoch,
            options: vec![],
            query: String::new(),
            matches: vec![],
            selected: None,
            loading: true,
            failed: false,
        }
    }
    fn filter(&mut self) {
        self.matches = self
            .options
            .iter()
            .enumerate()
            .filter_map(|(index, option)| {
                let label = option.label.to_lowercase();
                self.query
                    .split_whitespace()
                    .all(|word| label.contains(word))
                    .then_some(index)
            })
            .collect();
        self.selected = None;
    }
    pub(super) fn install(&mut self, result: anyhow::Result<Vec<ConnectionOption>>) {
        self.loading = false;
        self.failed = result.is_err();
        self.options = result.unwrap_or_default();
        self.filter();
    }
    pub(super) fn is_empty(&self) -> bool {
        self.matches.is_empty()
    }
}

impl ListDelegate for ConnectionPicker {
    type Item = ListItem;
    fn perform_search(
        &mut self,
        query: &str,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.query = query.to_lowercase();
        self.filter();
        cx.notify();
        Task::ready(())
    }
    fn items_count(&self, _: usize, _: &App) -> usize {
        self.matches.len()
    }
    fn loading(&self, _: &App) -> bool {
        self.loading
    }
    fn render_item(
        &mut self,
        index: IndexPath,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<ListItem> {
        let option = self.options.get(*self.matches.get(index.row)?)?;
        Some(
            ListItem::new(index.row)
                .h(px(32.))
                .disabled(self.loading || self.failed)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_sm()
                        .truncate()
                        .child(option.label.clone()),
                )
                .when(option.replace, |row| {
                    row.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(translate("canvas.connection.feedback.replace")),
                    )
                }),
        )
    }
    fn set_selected_index(
        &mut self,
        index: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) {
        self.selected = index;
    }
    fn confirm(&mut self, _: bool, _: &mut Window, cx: &mut Context<ListState<Self>>) {
        if self.loading || self.failed {
            return;
        }
        let Some(option) = self
            .selected
            .and_then(|index| self.matches.get(index.row))
            .and_then(|index| self.options.get(*index))
        else {
            return;
        };
        let source = self.source.clone();
        let target = option.address.clone();
        let direction = self.direction;
        let epoch = self.epoch;
        let picker_id = cx.entity_id();
        let _ = self.owner.update(cx, |view, cx| {
            if !view.accepts_input(epoch, cx)
                || view
                    .connection_picker
                    .as_ref()
                    .is_none_or(|(_, picker)| picker.entity_id() != picker_id)
            {
                return;
            }
            let (output, input) = if direction == PortDirection::Output {
                (source, target)
            } else {
                (target, source)
            };
            view.connection_picker = None;
            view.submit(
                GraphCommand::Edit(EditorGraphMutation::Connect {
                    output,
                    input,
                    order: None,
                }),
                cx,
            );
            cx.notify();
        });
    }
    fn cancel(&mut self, _: &mut Window, cx: &mut Context<ListState<Self>>) {
        let picker_id = cx.entity_id();
        let _ = self.owner.update(cx, |view, cx| {
            if view
                .connection_picker
                .as_ref()
                .is_some_and(|(_, picker)| picker.entity_id() == picker_id)
            {
                view.connection_picker = None;
                cx.notify();
            }
        });
    }
    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        let owner = self.owner.clone();
        let epoch = self.epoch;
        let picker_id = cx.entity_id();
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .items_center()
            .justify_center()
            .p_3()
            .text_sm()
            .child(translate(if self.failed {
                "detail.nodeDoc.connectionFailed"
            } else {
                "canvas.nodePalette.noMatches"
            }))
            .when(self.failed, |view| {
                view.child(
                    Button::new("retry-connections")
                        .small()
                        .ghost()
                        .label(translate("common.retry"))
                        .on_click(move |_, window, cx| {
                            let _ = owner.update(cx, |view, cx| {
                                if view.accepts_input(epoch, cx)
                                    && view
                                        .connection_picker
                                        .as_ref()
                                        .is_some_and(|(_, picker)| picker.entity_id() == picker_id)
                                {
                                    view.load_connection_picker(window, cx);
                                }
                            });
                        }),
                )
            })
    }
}
