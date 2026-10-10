//! Input and output groups mirror the reference panel's initially collapsed sections.
use super::*;
use crate::text::translate;
use gpui_kit::assets::IconName;
use gpui_kit::component::collapsible::Collapsible;
use yss_graph_editor::projection::EditorNodeModel;

impl DetailsPanel {
    pub(in crate::workbench::details) fn render_ports(
        &self,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(node) = self.node() else {
            return div().into_any_element();
        };
        let epoch = self.epoch;
        let mut groups = div().p_4().flex().flex_col().gap_3();
        for (section, direction) in [PortDirection::Input, PortDirection::Output]
            .into_iter()
            .enumerate()
        {
            let open = self.ports_open[section];
            let header = Button::new(("port-section", section))
                .small()
                .ghost()
                .w_full()
                .justify_start()
                .icon(if open {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                })
                .label(translate(if section == 0 {
                    "detail.nodeDoc.inputs"
                } else {
                    "detail.nodeDoc.outputs"
                }))
                .on_click(cx.listener(move |view, _, _, cx| {
                    if view.epoch == epoch {
                        view.ports_open[section] = !view.ports_open[section];
                        if !view.ports_open[section] {
                            view.connection_picker = None;
                        }
                        cx.notify();
                    }
                }));
            groups = groups.child(
                Collapsible::new()
                    .open(open)
                    .child(header)
                    .when(open, |section| {
                        section.content(self.render_port_group(node, direction, busy, cx))
                    }),
            );
        }
        groups.into_any_element()
    }

    fn render_port_group(
        &self,
        node: &EditorNodeModel,
        direction: PortDirection,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let epoch = self.epoch;
        let mut empty = true;
        let mut group = div().flex().flex_col().gap_2();
        for (index, field) in self
            .ports
            .iter()
            .enumerate()
            .filter(|(_, field)| field.model.direction == direction)
        {
            empty = false;
            group = group.child(self.render_port_field(index, field, busy, cx));
        }
        for (index, addition) in node
            .port_instance_additions
            .iter()
            .enumerate()
            .filter(|(_, addition)| addition.direction == direction)
        {
            empty = false;
            let node_id = node.node_id;
            let template_key = addition.template_key.clone();
            group = group.child(
                Button::new(("add-port", index))
                    .small()
                    .ghost()
                    .icon(IconName::Plus)
                    .label(crate::text::format(
                        "detail.nodeDoc.addPort",
                        &[("name", addition.label.to_string())],
                    ))
                    .disabled(busy || !addition.can_add)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if view.accepts_input(epoch, cx) {
                            view.submit(
                                GraphCommand::Edit(EditorGraphMutation::AddPortInstance {
                                    node_id,
                                    template_key: template_key.clone(),
                                    placement: PortPlacement::Append,
                                }),
                                cx,
                            );
                        }
                    })),
            );
        }
        group
            .when(empty, |view| {
                view.child(controls::hint(
                    translate(if direction == PortDirection::Input {
                        "detail.nodeDoc.noInputs"
                    } else {
                        "detail.nodeDoc.noOutputs"
                    }),
                    cx,
                ))
            })
            .into_any_element()
    }
}
