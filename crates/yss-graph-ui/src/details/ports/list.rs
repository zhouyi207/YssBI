//! Input and output groups mirror the reference panel's initially collapsed sections.
use super::*;
use yss_graph_editor::projection::EditorNodeModel;
use yss_node_protocol::PortDirection;
use yss_ui::inspector::InspectorSection;

impl GraphDetails {
    pub(in crate::details) fn render_ports(
        &self,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(node) = self.node() else {
            return div().into_any_element();
        };
        let epoch = self.epoch;
        let mut sections = div().min_w_0().flex().flex_col();
        for (section, direction, title) in [
            (0, PortDirection::Input, "detail.nodeDoc.inputs"),
            (1, PortDirection::Output, "detail.nodeDoc.outputs"),
        ] {
            let open = self.ports_open[section];
            let count = self
                .ports
                .iter()
                .filter(|field| field.model.direction == direction)
                .count();
            sections = sections.child(
                InspectorSection::new(("port-section", section), translate(title))
                    .count(count)
                    .collapsible(
                        open,
                        cx.listener(move |view, open: &bool, _, cx| {
                            if view.epoch == epoch {
                                view.ports_open[section] = *open;
                                if !*open {
                                    view.connection_picker = None;
                                }
                                cx.notify();
                            }
                        }),
                    )
                    .when(open, |content| {
                        content.child(self.render_port_group(node, direction, busy, cx))
                    }),
            );
        }
        sections.into_any_element()
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
        let mut group = div().min_w_0().flex().flex_col().gap_4();
        for (index, field) in self
            .ports
            .iter()
            .enumerate()
            .filter(|(_, field)| field.model.direction == direction)
        {
            empty = false;
            group = group.child(self.render_port_field(index, field, busy, cx));
        }
        let mut additions = Vec::new();
        for (index, addition) in node
            .port_instance_additions
            .iter()
            .enumerate()
            .filter(|(_, addition)| addition.direction == direction)
        {
            empty = false;
            let node_id = node.node_id;
            let template_key = addition.template_key.clone();
            additions.push(
                inspector::add_button(
                    ("add-port", index),
                    format(
                        "detail.nodeDoc.addPort",
                        &[("name", addition.label.to_string())],
                    ),
                )
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
            .when(!additions.is_empty(), |group| {
                group.child(div().flex().flex_col().items_start().children(additions))
            })
            .when(empty, |group| {
                group.child(inspector::hint(
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
