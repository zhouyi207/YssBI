//! Port commands use projected capabilities and the shared graph popup lifecycle.
use crate::canvas::{GraphCanvas, GraphCommand};
use gpui::{Context, Pixels, Point, Window};
use gpui_component::menu::PopupMenuItem;
use gpui_kit_assets::IconName;
use yss_graph_document::PortAddress;
use yss_graph_editor::EditorGraphMutation;

impl GraphCanvas {
    pub(in crate::canvas) fn show_port_menu(
        &mut self,
        address: PortAddress,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        if !self.can_edit() {
            return;
        }
        let after_address = address.clone();
        if self.commit_port_inputs_before(window, cx, move |view, window, cx| {
            view.show_port_menu(after_address, position, window, cx)
        }) {
            return;
        }
        let Some(port) = self
            .port_details
            .as_ref()
            .and_then(|details| details.port(&address))
        else {
            return;
        };
        let links = port.connections.current > 0;
        let reset = super::scalar_input_type(port).is_some()
            && port
                .input
                .as_ref()
                .is_some_and(|input| input.literal_override.is_some());
        let outputs = self.port_result_outputs(port);
        let previous = !self.previous_port_results(&outputs).is_empty();
        let view = !outputs.is_empty();
        self.cancel_gesture();
        self.palette = None;
        self.connection_click = None;
        self.located_port = Some(address.clone());
        let owner = cx.entity().downgrade();
        let projection = self.graph.projection.clone();
        let version = self.graph.editing.version;
        let language = crate::text::locale();
        let menu_address = address.clone();
        self.show_context_menu(position, window, cx, move |mut menu, _, cx| {
            let id = cx.entity_id();
            for (command, enabled) in [
                (PortAction::Disconnect, links),
                (PortAction::Reset, reset),
                (PortAction::View, view),
                (PortAction::Previous, previous),
            ] {
                if !enabled && matches!(command, PortAction::View | PortAction::Previous) {
                    continue;
                }
                let (label, icon) = match command {
                    PortAction::Disconnect => ("contextMenu.pin.breakLinks", IconName::Unlink),
                    PortAction::Reset => ("contextMenu.pin.resetValue", IconName::RotateCcw),
                    PortAction::View => ("contextMenu.pin.view", IconName::Eye),
                    PortAction::Previous => ("contextMenu.pin.viewPrevious", IconName::Clock),
                };
                let owner = owner.clone();
                let projection = projection.clone();
                let address = address.clone();
                menu = menu.item(
                    PopupMenuItem::new(crate::text::t(label))
                        .icon(icon)
                        .disabled(!enabled)
                        .on_click(move |_, _, cx| {
                            let _ = owner.update(cx, |view, cx| {
                                if !enabled
                                    || !view.menu_is_current(id, &projection, version, language)
                                {
                                    return;
                                }
                                match command {
                                    PortAction::Disconnect => view.submit(
                                        GraphCommand::Edit(EditorGraphMutation::DisconnectPort {
                                            address: address.clone(),
                                        }),
                                        Some(version),
                                        cx,
                                    ),
                                    PortAction::Reset => view.submit(
                                        GraphCommand::Edit(EditorGraphMutation::SetLiteral {
                                            address: address.clone(),
                                            literal: None,
                                        }),
                                        Some(version),
                                        cx,
                                    ),
                                    PortAction::View | PortAction::Previous => view
                                        .inspect_port_result(
                                            &address,
                                            matches!(command, PortAction::Previous),
                                            cx,
                                        ),
                                }
                            });
                        }),
                );
            }
            menu
        });
        if let Some(menu) = &mut self.context_menu {
            menu.port = Some(menu_address);
        }
    }
}

#[derive(Clone, Copy)]
enum PortAction {
    Disconnect,
    Reset,
    View,
    Previous,
}
