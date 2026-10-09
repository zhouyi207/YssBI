//! Port connection views consume the same projection and transactions as the canvas.
mod picker;

use super::{DetailsPanel, ports::PortField};
use crate::{canvas::GraphCommand, text::translate};
use gpui::{AnyElement, Context, Focusable, IntoElement, Window, div, prelude::*, px};
use gpui_component::{
    Disableable, IndexPath, Sizable,
    button::{Button, ButtonVariants},
    list::{List, ListState},
    popover::Popover,
    tooltip::Tooltip,
};
use gpui_kit_assets::IconName;
use std::collections::{BTreeMap, BTreeSet};
use yss_graph_document::{ConnectionId, PortAddress};
use yss_graph_editor::{
    EditorGraphMutation,
    projection::{ConnectionIntent, EditorProjectionModel},
};
use yss_node_protocol::PortDirection;

pub(super) use picker::ConnectionPicker;
const PAGE_CONNECTIONS: usize = 50;

pub(super) struct ConnectedPort {
    id: ConnectionId,
    label: String,
}

fn port_labels<'a>(
    projection: &'a EditorProjectionModel,
    wanted: &BTreeSet<&PortAddress>,
) -> BTreeMap<&'a PortAddress, String> {
    projection
        .nodes
        .iter()
        .flat_map(|node| {
            let title = node
                .display
                .user_label
                .as_deref()
                .unwrap_or(&node.display.title);
            node.ports
                .iter()
                .filter(|port| wanted.contains(&port.address))
                .map(move |port| {
                    let name = port
                        .display
                        .instance_label
                        .as_deref()
                        .unwrap_or(&port.display.label);
                    (&port.address, format!("{title} · {name}"))
                })
        })
        .collect()
}

impl DetailsPanel {
    pub(super) fn install_connections(&mut self) {
        let Some(projection) = &self.projection else {
            return;
        };
        let Some(node) = self.node() else { return };
        let node_id = node.node_id;
        let wanted = projection
            .connections
            .iter()
            .filter_map(|connection| {
                if connection.input.node_id == node_id {
                    Some(&connection.output)
                } else if connection.output.node_id == node_id {
                    Some(&connection.input)
                } else {
                    None
                }
            })
            .collect();
        let labels = port_labels(projection, &wanted);
        let mut peers: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for connection in &projection.connections {
            for (local, peer) in [
                (&connection.input, &connection.output),
                (&connection.output, &connection.input),
            ] {
                if local.node_id == node_id {
                    peers.entry(local).or_default().push(ConnectedPort {
                        id: connection.connection_id,
                        label: labels
                            .get(peer)
                            .cloned()
                            .unwrap_or_else(|| translate("detail.nodeDoc.unnamed")),
                    });
                }
            }
        }
        for field in &mut self.ports {
            field.peers = peers.remove(&field.model.address).unwrap_or_default();
            field.connections_page = field
                .connections_page
                .min(field.peers.len().saturating_sub(1) / PAGE_CONNECTIONS);
        }
    }

    pub(super) fn load_connection_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((source, picker)) = self.connection_picker.clone() else {
            return;
        };
        if !self.accepts_input(self.epoch, cx) {
            return;
        }
        let Some(graph) = self.graph() else { return };
        let graph = graph.read(cx);
        let project = graph.graph.project.clone();
        let path = graph.graph.projection.graph_path.clone();
        let projection = graph.graph.projection.clone();
        let version = graph.graph.editing.version;
        let epoch = self.epoch;
        let picker_id = picker.entity_id();
        picker.update(cx, |list, cx| {
            list.delegate_mut().loading = true;
            list.delegate_mut().failed = false;
            cx.notify();
        });
        let job = self.services.run(move |services| {
            let candidates = services.application.graph_connection_candidates(
                &project,
                &path,
                version,
                &source,
                ConnectionIntent::Connect,
            )?;
            anyhow::ensure!(
                candidates.semantic_input_hash == projection.basis.semantic_input_hash,
                "connection projection changed"
            );
            Ok(picker::options(candidates, &projection))
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update_in(cx, |view, window, cx| {
                if !view.accepts_input(epoch, cx)
                    || view
                        .connection_picker
                        .as_ref()
                        .is_none_or(|(_, current)| current.entity_id() != picker_id)
                {
                    return;
                }
                picker.update(cx, |list, cx| {
                    list.delegate_mut().install(result);
                    let selected = (!list.delegate().is_empty()).then_some(IndexPath::default());
                    list.set_selected_index(selected, window, cx);
                    cx.notify();
                });
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn render_port_connections(
        &self,
        index: usize,
        field: &PortField,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let epoch = self.epoch;
        let source = field.model.address.clone();
        let direction = field.model.direction;
        let picker = self
            .connection_picker
            .as_ref()
            .filter(|(current, _)| current == &source)
            .map(|(_, picker)| picker.clone());
        let content = picker.clone();
        let disabled = busy
            || field.model.orphan
            || !(field.model.connections.can_append || field.model.connections.can_replace);
        let popover = Popover::new(("port-connect-picker", index))
            .open(picker.is_some())
            .trigger(
                Button::new(("port-connect", index))
                    .small()
                    .ghost()
                    .icon(IconName::Plus)
                    .label(translate("detail.nodeDoc.addConnection"))
                    .disabled(disabled),
            )
            .when_some(picker, |popover, picker| {
                popover.track_focus(&picker.focus_handle(cx))
            })
            .on_open_change(cx.listener(move |view, open: &bool, window, cx| {
                if *open {
                    if disabled || !view.accepts_input(epoch, cx) {
                        return;
                    }
                    let owner = cx.entity().downgrade();
                    let delegate = ConnectionPicker::new(owner, source.clone(), direction, epoch);
                    let picker = cx.new(|cx| ListState::new(delegate, window, cx).searchable(true));
                    picker.update(cx, |list, cx| list.focus(window, cx));
                    view.connection_picker = Some((source.clone(), picker));
                    view.load_connection_picker(window, cx);
                } else if view.epoch == epoch
                    && view
                        .connection_picker
                        .as_ref()
                        .is_some_and(|(current, _)| current == &source)
                {
                    view.connection_picker = None;
                }
                cx.notify();
            }))
            .content(move |_, _, _| {
                div()
                    .w(px(320.))
                    .h(px(300.))
                    .when_some(content.clone(), |view, picker| {
                        view.child(List::new(&picker))
                    })
            });
        let page = field.connections_page;
        let pages = field.peers.len().div_ceil(PAGE_CONNECTIONS).max(1);
        div()
            .flex()
            .flex_col()
            .gap_1()
            .when(field.peers.is_empty(), |view| {
                view.child(super::controls::hint(
                    translate("detail.nodeDoc.unconnected"),
                    cx,
                ))
            })
            .children(
                field
                    .peers
                    .iter()
                    .skip(page * PAGE_CONNECTIONS)
                    .take(PAGE_CONNECTIONS)
                    .map(|peer| {
                        let id = peer.id;
                        let tooltip = peer.label.clone();
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .id(gpui::SharedString::from(format!("connection-peer-{id}")))
                                    .flex_1()
                                    .min_w_0()
                                    .text_sm()
                                    .truncate()
                                    .child(peer.label.clone())
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(tooltip.clone()).build(window, cx)
                                    }),
                            )
                            .child(
                                Button::new(gpui::SharedString::from(format!(
                                    "remove-connection-{id}"
                                )))
                                .small()
                                .ghost()
                                .icon(IconName::Minus)
                                .tooltip(translate("detail.nodeDoc.removeConnection"))
                                .disabled(busy)
                                .on_click(cx.listener(
                                    move |view, _, _, cx| {
                                        if view.accepts_input(epoch, cx) {
                                            view.submit(
                                                GraphCommand::Edit(
                                                    EditorGraphMutation::DisconnectConnections {
                                                        connection_ids: vec![id],
                                                    },
                                                ),
                                                cx,
                                            );
                                        }
                                    },
                                )),
                            )
                    }),
            )
            .when(pages > 1, |view| {
                view.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            Button::new(("peer-prev", index))
                                .small()
                                .ghost()
                                .icon(IconName::ChevronLeft)
                                .tooltip(translate("databaseEditor.previousPage"))
                                .disabled(page == 0)
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    if view.epoch == epoch {
                                        view.ports[index].connections_page = page.saturating_sub(1);
                                        cx.notify();
                                    }
                                })),
                        )
                        .child(div().text_xs().child(format!("{} / {pages}", page + 1)))
                        .child(
                            Button::new(("peer-next", index))
                                .small()
                                .ghost()
                                .icon(IconName::ChevronRight)
                                .tooltip(translate("databaseEditor.nextPage"))
                                .disabled(page + 1 >= pages)
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    if view.epoch == epoch {
                                        view.ports[index].connections_page =
                                            (page + 1).min(pages - 1);
                                        cx.notify();
                                    }
                                })),
                        ),
                )
            })
            .child(popover)
            .into_any_element()
    }
}
