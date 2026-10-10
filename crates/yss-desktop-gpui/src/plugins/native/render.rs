use super::{PluginViewPanel, fields::Draft};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::Button,
    checkbox::Checkbox,
    input::{Input, Textarea},
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit::{AnyElement, Context, IntoElement, Render, Window, div, prelude::*};
use yss_plugin_runtime::{NativeInput, NativeOperation};

impl Render for PluginViewPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut content = div()
            .id("native-plugin-view")
            .track_focus(&self.focus)
            .size_full()
            .overflow_y_scroll()
            .p_4()
            .flex()
            .flex_col()
            .gap_3();
        if let Some(model) = &self.model {
            let generation = self.form_generation;
            content = content.child(model.description.clone());
            for (index, field) in self.fields.iter().enumerate() {
                let control: AnyElement = match &field.draft {
                    Draft::Input(editor) => {
                        Input::new(editor).disabled(self.busy).into_any_element()
                    }
                    Draft::Multiline(editor) => {
                        Textarea::new(editor).disabled(self.busy).into_any_element()
                    }
                    Draft::Boolean(value) => Checkbox::new(("plugin-field", index))
                        .label(field.model.label.clone())
                        .checked(*value)
                        .disabled(self.busy)
                        .on_click(cx.listener(move |panel, value: &bool, _, cx| {
                            if !panel.busy && !panel.closed && panel.form_generation == generation {
                                panel.fields[index].draft = Draft::Boolean(*value);
                                cx.notify();
                            }
                        }))
                        .into_any_element(),
                    Draft::Choice(value) => {
                        let options =
                            if let NativeInput::Choice { options, .. } = &field.model.input {
                                options.clone()
                            } else {
                                vec![]
                            };
                        let current = value.clone();
                        let panel = cx.weak_entity();
                        Button::new(("plugin-field", index))
                            .label(value.clone())
                            .icon(IconName::ChevronDown)
                            .disabled(self.busy)
                            .dropdown_menu(move |mut menu, _, _| {
                                menu = menu.scrollable(true);
                                for option in &options {
                                    let panel = panel.clone();
                                    let selected = option.clone();
                                    menu = menu.item(
                                        PopupMenuItem::new(option.clone())
                                            .checked(*option == current)
                                            .on_click(move |_, _, cx| {
                                                let _ = panel.update(cx, |panel, cx| {
                                                    if !panel.busy
                                                        && !panel.closed
                                                        && panel.form_generation == generation
                                                    {
                                                        panel.fields[index].draft =
                                                            Draft::Choice(selected.clone());
                                                        cx.notify();
                                                    }
                                                });
                                            }),
                                    );
                                }
                                menu
                            })
                            .into_any_element()
                    }
                };
                content = content.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(div().text_sm().child(field.model.label.clone()))
                        .child(control),
                );
            }
            let mut actions = div().flex().flex_wrap().gap_2();
            for (index, action) in model.actions.iter().enumerate() {
                let operation = action.operation.clone();
                let running = matches!(operation, NativeOperation::StartTask { .. })
                    && self
                        .task
                        .as_ref()
                        .is_some_and(|task| !task.state.terminal());
                actions = actions.child(
                    Button::new(("plugin-action", index))
                        .small()
                        .label(action.label.clone())
                        .disabled(self.busy || running)
                        .on_click(cx.listener(move |panel, _, window, cx| {
                            if panel.form_generation == generation {
                                panel.execute(operation.clone(), window, cx);
                            }
                        })),
                );
            }
            if self.allows("views.set_state") {
                actions = actions.child(
                    Button::new("plugin-save-state")
                        .small()
                        .label(crate::text::t("native.plugins.saveForm"))
                        .disabled(self.busy)
                        .on_click(cx.listener(|panel, _, window, cx| panel.save(window, cx))),
                );
            }
            content = content.child(actions);
        }
        if let Some(task) = &self.task {
            content = content.child(
                div()
                    .text_sm()
                    .child(super::super::details::task_state(task.state)),
            );
            if let Some(error) = &task.error {
                content = content.child(super::super::failure(error));
            }
            if let Some(message) = task
                .result
                .as_ref()
                .and_then(|value| value.get("viewData"))
                .and_then(|value| value.get("message"))
                .and_then(serde_json::Value::as_str)
            {
                content = content.child(message.to_owned());
            }
            if !task.state.terminal() && self.allows("tasks.cancel") {
                content = content.child(
                    Button::new("plugin-cancel-task")
                        .small()
                        .label(crate::text::t("common.cancel"))
                        .disabled(self.busy)
                        .on_click(cx.listener(|panel, _, window, cx| panel.cancel(window, cx))),
                );
            }
        }
        if self.busy {
            content = content.child(crate::text::t("common.loading"));
        }
        if let Some(message) = &self.message {
            content = content.child(message.clone());
        }
        if let Some(error) = &self.error {
            content = content.child(
                div()
                    .text_color(cx.theme().danger)
                    .child(super::super::failure(error)),
            );
        }
        content
    }
}
