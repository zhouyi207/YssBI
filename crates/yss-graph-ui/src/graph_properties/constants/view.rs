//! Native constant records, type controls and graph-scoped drag handles.
use super::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, Sizable,
    button::Button,
    checkbox::Checkbox,
    input::Input,
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit::{Anchor, AnyElement, IntoElement, div, prelude::*};
use yss_ui::inspector;
use yss_ui::text::translate;

impl GraphProperties {
    pub(super) fn render_constant(
        &self,
        row: usize,
        field: &ConstantDraft,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(name) = field.name.as_ref() else {
            return div().into_any_element();
        };
        let generation = self.generation;
        let id = field.model.id;
        let disabled = busy || field.value_loading;
        let identity = div()
            .flex()
            .items_center()
            .gap_1()
            .children(
                (!disabled)
                    .then(|| self.render_constant_drag(row, field, cx))
                    .flatten(),
            )
            .child(
                Input::new(name)
                    .small()
                    .flex_1()
                    .min_w_0()
                    .disabled(disabled),
            )
            .child(self.render_constant_menu(row, id, disabled, cx));
        let data_type = value::type_menu(
            inspector::choice_trigger(("constant-type", row), field.data_type.to_string())
                .disabled(disabled),
            Some(field.data_type.to_string()),
            cx.listener(move |view, source: &String, window, cx| {
                if !view.accepts_input(generation, cx) {
                    return;
                }
                if let Some(field) = view.constants.iter_mut().find(|field| field.model.id == id)
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
        );
        let mut content = inspector::record(cx)
            .child(identity)
            .child(data_type)
            .child(
                Checkbox::new(("constant-null", row))
                    .label(translate("native.workbench.nullValue"))
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
                content = content.child(inspector::hint(translate(key), cx));
            }
            if field.data_type == ValueType::Scalar(SemanticType::Binary) {
                content = content.child(
                    Checkbox::new(("constant-bool", row))
                        .label(translate("native.workbench.trueValue"))
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
                    .outline()
                    .icon(IconName::Settings2)
                    .label(yss_ui::text::format(
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
                div().flex().justify_end().child(
                    inspector::commit_button(
                        ("apply-constant", row),
                        translate("native.workbench.applyChanges"),
                        disabled,
                    )
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if view.accepts_input(generation, cx) {
                            view.apply_constant(id, cx);
                        }
                    })),
                ),
            )
            .into_any_element()
    }

    fn render_constant_drag(
        &self,
        row: usize,
        field: &ConstantDraft,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let graph = self.graph()?;
        let version = self.version?;
        let graph = graph.read(cx);
        let drag = crate::canvas::ConstantDrag {
            project: graph.graph.project.clone(),
            path: graph.graph.projection.graph_path.clone(),
            version,
            id: field.model.id,
            name: field.model.name.clone(),
        };
        Some(
            div()
                .id(("drag-constant", row))
                .flex_shrink_0()
                .p_0p5()
                .rounded_sm()
                .cursor_grab()
                .text_color(cx.theme().muted_foreground)
                .hover(|handle| handle.bg(cx.theme().muted))
                .child(Icon::new(IconName::GripVertical).xsmall())
                .on_drag(drag, |drag, _, _, cx| {
                    cx.stop_propagation();
                    cx.new(|_| drag.clone())
                })
                .into_any_element(),
        )
    }

    fn render_constant_menu(
        &self,
        row: usize,
        id: ConstantId,
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let generation = self.generation;
        let owner = cx.entity().downgrade();
        inspector::overflow_button(("constant-actions", row))
            .disabled(disabled)
            .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, _| {
                menu.item(
                    PopupMenuItem::new(translate("common.restore"))
                        .icon(IconName::RotateCcw)
                        .on_click(inspector::owner_command(
                            owner.clone(),
                            move |view: &mut GraphProperties, _, cx| {
                                if view.accepts_input(generation, cx)
                                    && let Some(field) =
                                        view.constants.iter_mut().find(|field| field.model.id == id)
                                {
                                    *field = ConstantDraft::new(field.model.clone());
                                    cx.notify();
                                }
                            },
                        )),
                )
                .item(
                    PopupMenuItem::new(translate("native.workbench.insertConstantReference"))
                        .icon(IconName::Plus)
                        .on_click(inspector::owner_command(
                            owner.clone(),
                            move |view: &mut GraphProperties, _, cx| {
                                if view.accepts_input(generation, cx)
                                    && let Some(graph) = view.graph()
                                    && let Some(version) = view.version
                                {
                                    graph.update(cx, |graph, cx| {
                                        graph.insert_constant_reference(id, version, cx)
                                    });
                                }
                            },
                        )),
                )
                .separator()
                .item(
                    PopupMenuItem::new(translate("native.workbench.deleteConstant"))
                        .icon(IconName::Trash)
                        .on_click(inspector::owner_command(
                            owner.clone(),
                            move |view: &mut GraphProperties, _, cx| {
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
                            },
                        )),
                )
            })
            .into_any_element()
    }
}
