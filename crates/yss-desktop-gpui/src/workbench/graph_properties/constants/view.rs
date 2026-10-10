//! Native constant cards, type controls and graph-scoped drag handles.
use super::*;
use crate::workbench::controls;
use gpui::{AnyElement, IntoElement, div, prelude::*};
use gpui_component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    input::Input,
};
use gpui_kit_assets::IconName;

impl GraphProperties {
    pub(super) fn render_constant(
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
            .child(
                Input::new(field.name.as_ref().expect("visible constant input"))
                    .small()
                    .disabled(disabled),
            )
            .child(value::type_picker(
                ("constant-type", row),
                field.data_type.to_string(),
                Some(field.data_type.to_string()),
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
                    .label(crate::text::t("native.workbench.nullValue"))
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
            if let Some(key) = value::json_hint(&field.data_type) {
                content = content.child(controls::hint(crate::text::translate(key), cx));
            }
            if field.data_type == ValueType::Scalar(SemanticType::Binary) {
                content = content.child(
                    Checkbox::new(("constant-bool", row))
                        .label(crate::text::t("native.workbench.trueValue"))
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
                    .label(crate::text::format(
                        "native.workbench.editConstantValue",
                        &[("value0", field.model.summary.label())],
                    ))
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
                            .tooltip(crate::text::t("native.workbench.applyConstant"))
                            .on_click(cx.listener(move |view, _, _, cx| {
                                if view.accepts_input(generation, cx) {
                                    view.apply_constant(id, cx);
                                }
                            })),
                    )
                    .child(
                        Button::new(("restore-constant", row))
                            .small()
                            .ghost()
                            .icon(IconName::Undo2)
                            .tooltip(crate::text::translate("common.restore"))
                            .disabled(disabled)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                if view.accepts_input(generation, cx)
                                    && let Some(field) =
                                        view.constants.iter_mut().find(|field| field.model.id == id)
                                {
                                    *field = ConstantDraft::new(field.model.clone());
                                    view.error = None;
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        Button::new(("constant-reference", row))
                            .small()
                            .ghost()
                            .icon(IconName::Plus)
                            .tooltip(crate::text::t("native.workbench.insertConstantReference"))
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
                            .tooltip(crate::text::t("native.workbench.deleteConstant"))
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
