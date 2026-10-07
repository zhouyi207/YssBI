//! Native constant cards, type controls and graph-scoped drag handles.
use super::*;
use crate::workbench::controls;
use gpui::{AnyElement, IntoElement, div, prelude::*};
use gpui_component::{
    ActiveTheme, Disableable, IconName, Sizable,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    input::Input,
};

impl GraphProperties {
    pub(in crate::workbench::graph_properties) fn render_constants(
        &self,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let generation = self.generation;
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("图常量"),
                    )
                    .child(
                        Button::new("add-graph-constant")
                            .small()
                            .ghost()
                            .icon(IconName::Plus)
                            .tooltip("创建常量")
                            .disabled(busy)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                if view.accepts_input(generation, cx) {
                                    view.add_constant(cx);
                                }
                            })),
                    ),
            )
            .when(self.constants.is_empty(), |view| {
                view.child(controls::hint("添加常量后可在画布插入引用节点", cx))
            })
            .children(
                self.constants
                    .iter()
                    .enumerate()
                    .map(|(row, field)| self.render_constant(row, field, busy, cx)),
            )
            .into_any_element()
    }

    fn render_constant(
        &self,
        row: usize,
        field: &ConstantDraft,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let generation = self.generation;
        let id = field.model.id;
        let disabled = busy || field.value_loading;
        let mut content = div()
            .flex()
            .flex_col()
            .gap_2()
            .p_2()
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .child(Input::new(&field.name).small().disabled(disabled))
            .child(controls::choice(
                ("constant-type", row),
                field.data_type.to_string(),
                Some(field.data_type.to_string()),
                value::type_options(),
                disabled,
                cx.listener(move |view, source: &String, window, cx| {
                    if !view.accepts_input(generation, cx) {
                        return;
                    }
                    if let Some(field) =
                        view.constants.iter_mut().find(|field| field.model.id == id)
                        && let Ok(data_type) = source.parse::<ValueType>()
                    {
                        if field.data_type == data_type {
                            return;
                        }
                        field.input = Some(input_state(
                            value::initial_value(&data_type).into(),
                            &data_type,
                            window,
                            cx,
                        ));
                        field.is_null = data_type == ValueType::Scalar(SemanticType::Ordinal);
                        field.data_type = data_type;
                        field.value_changed_type = true;
                        field.load_token = field.load_token.wrapping_add(1);
                        field.value_loading = false;
                        cx.notify();
                    }
                }),
            ))
            .child(
                Checkbox::new(("constant-null", row))
                    .label("空值")
                    .checked(field.is_null)
                    .disabled(disabled)
                    .on_click(cx.listener(move |view, value: &bool, _, cx| {
                        if view.accepts_input(generation, cx)
                            && let Some(field) =
                                view.constants.iter_mut().find(|field| field.model.id == id)
                        {
                            field.is_null = *value;
                            cx.notify();
                        }
                    })),
            );
        if let Some(input) = &field.input {
            if matches!(field.data_type, ValueType::Array(_) | ValueType::Object) {
                content = content.child(controls::hint(
                    "类型化 JSON：整数、小数使用字符串保留精度",
                    cx,
                ));
            }
            if field.data_type == ValueType::Scalar(SemanticType::Binary) {
                content = content.child(
                    Checkbox::new(("constant-bool", row))
                        .label("真")
                        .checked(input.value(cx) == "true")
                        .disabled(disabled || field.is_null)
                        .on_click(cx.listener(move |view, value: &bool, window, cx| {
                            if view.accepts_input(generation, cx)
                                && let Some(field) =
                                    view.constants.iter_mut().find(|field| field.model.id == id)
                                && let Some(input) = &field.input
                            {
                                input.set_value(value.to_string(), window, cx);
                            }
                        })),
                );
            } else {
                content = content.child(input.render(disabled || field.is_null));
            }
        } else {
            content = content.child(
                Button::new(("load-constant-value", row))
                    .small()
                    .ghost()
                    .icon(IconName::Settings2)
                    .label(format!("编辑值 · {}", field.model.summary))
                    .disabled(disabled || field.is_null)
                    .on_click(cx.listener(move |view, _, window, cx| {
                        if view.accepts_input(generation, cx) {
                            view.load_constant_value(id, window, cx);
                        }
                    })),
            );
        }
        content
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .when(!disabled, |view| {
                        if let Some(graph) = self.graph()
                            && let Some(version) = self.version
                        {
                            let graph = graph.read(cx);
                            let drag = crate::canvas::ConstantDrag {
                                project: graph.graph.project.clone(),
                                path: graph.graph.projection.graph_path.clone(),
                                version,
                                id,
                                name: field.model.name.clone(),
                            };
                            view.child(
                                div()
                                    .id(("drag-constant", row))
                                    .p_1()
                                    .rounded_sm()
                                    .cursor_pointer()
                                    .hover(|view| view.bg(cx.theme().muted))
                                    .child(gpui_component::Icon::new(IconName::Menu).size_3())
                                    .on_drag(drag, |drag, _, _, cx| {
                                        cx.stop_propagation();
                                        cx.new(|_| drag.clone())
                                    }),
                            )
                        } else {
                            view
                        }
                    })
                    .child(
                        controls::apply(("apply-constant", row), disabled)
                            .tooltip("应用常量修改")
                            .on_click(cx.listener(move |view, _, _, cx| {
                                if view.accepts_input(generation, cx) {
                                    view.apply_constant(id, cx);
                                }
                            })),
                    )
                    .child(
                        Button::new(("constant-reference", row))
                            .small()
                            .ghost()
                            .icon(IconName::Plus)
                            .tooltip("在画布中心插入引用节点")
                            .disabled(disabled)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                if view.accepts_input(generation, cx)
                                    && let Some(graph) = view.graph()
                                    && let Some(version) = view.version
                                {
                                    graph.update(cx, |graph, cx| {
                                        graph.insert_constant_reference(id, version, cx)
                                    });
                                }
                            })),
                    )
                    .child(
                        Button::new(("delete-constant", row))
                            .small()
                            .ghost()
                            .icon(IconName::Close)
                            .tooltip("删除常量，引用节点保留")
                            .disabled(disabled)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                if view.accepts_input(generation, cx)
                                    && let Some(graph) = view.graph()
                                    && let Some(version) = view.version
                                {
                                    graph.update(cx, |graph, cx| {
                                        graph.submit(
                                            GraphCommand::Edit(EditorGraphMutation::SetConstant {
                                                id,
                                                constant: None,
                                            }),
                                            Some(version),
                                            cx,
                                        )
                                    });
                                }
                            })),
                    ),
            )
            .into_any_element()
    }
}
