use super::SettingsPanel;
use gpui_kit::component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::{Input, Textarea},
    setting::{SettingGroup, SettingItem},
};
use gpui_kit::{Context, IntoElement, SharedString, div, prelude::*};
use yss_harness_contract::ReasoningEffort;

impl SettingsPanel {
    pub(in crate::settings) fn provider_form(&self, cx: &mut Context<Self>) -> Vec<SettingGroup> {
        let Some(draft) = &self.editor else {
            return Vec::new();
        };
        let mut groups = vec![self.connection_fields(cx), self.model_list(cx)];
        if self.model.is_some() {
            groups.push(self.model_form(cx));
        }
        if draft.saved {
            groups.push(
                SettingGroup::new().item(
                    self.model_action_row(
                        |view, cx| {
                            div().flex().child(
                                Button::new("provider-delete")
                                    .small()
                                    .danger()
                                    .label(crate::text::translate("settings.models.removeProvider"))
                                    .disabled(view.busy())
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.delete_provider(window, cx)
                                    })),
                            )
                        },
                        cx,
                    )
                    .keywords([crate::text::translate("settings.models.removeProvider")]),
                ),
            );
        }
        groups
    }

    fn model_action_row<E: IntoElement>(
        &self,
        render: impl Fn(&Self, &mut Context<Self>) -> E + 'static,
        cx: &Context<Self>,
    ) -> SettingItem {
        let owner = cx.weak_entity();
        SettingItem::render(move |_, _, cx| {
            owner
                .update(cx, |view, cx| render(view, cx).into_any_element())
                .unwrap_or_else(|_| div().into_any_element())
        })
    }

    fn model_list(&self, cx: &mut Context<Self>) -> SettingGroup {
        let mut group =
            SettingGroup::new()
                .title(crate::text::t("bayes.tabs.model"))
                .item(
                    self.model_action_row(
                        |view, cx| {
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    Button::new("models-discover")
                                        .small()
                                        .ghost()
                                        .label(crate::text::t("native.settings.fetchModels"))
                                        .disabled(view.busy() || !view.connection_ready(cx))
                                        .on_click(cx.listener(|view, _, window, cx| {
                                            view.discover(window, cx)
                                        })),
                                )
                                .child(
                                    Button::new("models-add")
                                        .small()
                                        .ghost()
                                        .label(crate::text::t("native.settings.addModel"))
                                        .disabled(view.busy() || view.model.is_some())
                                        .on_click(cx.listener(|view, _, window, cx| {
                                            view.edit_model(None, window, cx)
                                        })),
                                )
                        },
                        cx,
                    )
                    .keywords([
                        crate::text::t("native.settings.fetchModels"),
                        crate::text::t("native.settings.addModel"),
                    ]),
                );
        let Some(draft) = &self.editor else {
            return group;
        };
        if draft.models.is_empty() {
            group = group.description(crate::text::t("native.settings.noModels"));
        }
        for (index, model) in draft.models.iter().enumerate() {
            let epoch = self.epoch;
            let model_id = model.id.clone();
            group = group.item(self.render_field(
                model.name.clone(),
                model.id.clone(),
                move |view, cx| {
                    if view.epoch != epoch
                        || !view.editor.as_ref().is_some_and(|draft| {
                            draft
                                .models
                                .get(index)
                                .is_some_and(|model| model.id == model_id)
                        })
                    {
                        return div().into_any_element();
                    }
                    let edit_id = model_id.clone();
                    let remove_id = model_id.clone();
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new(SharedString::from(format!("model-edit-{model_id}")))
                                .small()
                                .ghost()
                                .label(crate::text::t("detail.constantValue.edit"))
                                .disabled(view.busy())
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    if view.model_row_current(epoch, index, &edit_id) {
                                        view.edit_model(Some(index), window, cx);
                                    }
                                })),
                        )
                        .child(
                            Button::new(SharedString::from(format!("model-remove-{model_id}")))
                                .small()
                                .ghost()
                                .label(crate::text::t("native.workbench.remove"))
                                .disabled(view.busy() || view.model.is_some())
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    if !view.model_row_current(epoch, index, &remove_id)
                                        || view.model.is_some()
                                    {
                                        return;
                                    }
                                    if let Some(draft) = &mut view.editor {
                                        draft.models.remove(index);
                                        draft.changed = true;
                                    }
                                    cx.notify();
                                })),
                        )
                        .into_any_element()
                },
                cx,
            ));
        }
        group
    }

    fn model_form(&self, cx: &mut Context<Self>) -> SettingGroup {
        SettingGroup::new()
            .item(self.render_field(
                crate::text::t("settings.models.modelId"),
                crate::text::t("native.settings.modelIdHint"),
                |view, _| {
                    match &view.model {
                        Some(model) => Input::new(&model.id)
                            .disabled(view.busy())
                            .into_any_element(),
                        None => div().into_any_element(),
                    }
                },
                cx,
            ))
            .item(self.render_field(
                crate::text::t("settings.models.displayName"),
                crate::text::t("native.settings.modelNameHint"),
                |view, _| {
                    match &view.model {
                        Some(model) => Input::new(&model.name)
                            .disabled(view.busy())
                            .into_any_element(),
                        None => div().into_any_element(),
                    }
                },
                cx,
            ))
            .item(self.render_field(
                crate::text::t("native.settings.contextCapacity"),
                crate::text::t("native.settings.contextWindowHint"),
                |view, _| {
                    match &view.model {
                        Some(model) => Input::new(&model.context)
                            .disabled(view.busy())
                            .into_any_element(),
                        None => div().into_any_element(),
                    }
                },
                cx,
            ))
            .item(self.render_field(
                crate::text::t("native.settings.maxOutput"),
                crate::text::t("native.settings.maxOutputHint"),
                |view, _| {
                    match &view.model {
                        Some(model) => Input::new(&model.output)
                            .disabled(view.busy())
                            .into_any_element(),
                        None => div().into_any_element(),
                    }
                },
                cx,
            ))
            .item(self.render_field(
                "Temperature",
                crate::text::t("native.settings.serviceDefaultHint"),
                |view, _| {
                    match &view.model {
                        Some(model) => Input::new(&model.temperature)
                            .disabled(view.busy())
                            .into_any_element(),
                        None => div().into_any_element(),
                    }
                },
                cx,
            ))
            .item(self.render_field(
                "Top P",
                crate::text::t("native.settings.serviceDefaultHint"),
                |view, _| {
                    match &view.model {
                        Some(model) => Input::new(&model.top_p)
                            .disabled(view.busy())
                            .into_any_element(),
                        None => div().into_any_element(),
                    }
                },
                cx,
            ))
            .item(self.render_field(
                crate::text::t("native.settings.reasoningLevels"),
                crate::text::t("native.settings.reasoningHint"),
                |view, cx| {
                    let Some(model) = &view.model else {
                        return div().into_any_element();
                    };
                    let epoch = view.epoch;
                    let mut efforts = div().flex().gap_2().flex_wrap();
                    for (id, label, effort) in [
                        ("model-effort-low", "Low", ReasoningEffort::Low),
                        ("model-effort-medium", "Medium", ReasoningEffort::Medium),
                        ("model-effort-high", "High", ReasoningEffort::High),
                    ] {
                        efforts = efforts.child(
                            Button::new(id)
                                .small()
                                .ghost()
                                .label(label)
                                .toggled(model.efforts.contains(&effort))
                                .disabled(view.busy())
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    if view.busy() || view.epoch != epoch {
                                        return;
                                    }
                                    if let Some(model) = &mut view.model {
                                        if model.efforts.contains(&effort) {
                                            model.efforts.retain(|value| *value != effort);
                                        } else {
                                            model.efforts.push(effort);
                                        }
                                        model.changed = true;
                                        cx.notify();
                                    }
                                })),
                        );
                    }
                    efforts.into_any_element()
                },
                cx,
            ))
            .item(self.render_field(
                crate::text::t("native.settings.additionalParameters"),
                crate::text::t("native.settings.parametersHint"),
                |view, _| {
                    match &view.model {
                        Some(model) => Textarea::new(&model.parameters)
                            .disabled(view.busy())
                            .into_any_element(),
                        None => div().into_any_element(),
                    }
                },
                cx,
            ))
            .item(
                self.model_action_row(
                    |view, cx| {
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("model-cancel")
                                    .small()
                                    .ghost()
                                    .label(crate::text::t("native.settings.cancelModelEdit"))
                                    .disabled(view.busy())
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        if view.busy() {
                                            return;
                                        }
                                        view.model = None;
                                        view.model_subscriptions.clear();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("model-apply")
                                    .small()
                                    .primary()
                                    .label(crate::text::t("native.settings.applyToDraft"))
                                    .disabled(view.busy())
                                    .on_click(cx.listener(|view, _, _, cx| view.apply_model(cx))),
                            )
                    },
                    cx,
                )
                .keywords([
                    crate::text::t("native.settings.cancelModelEdit"),
                    crate::text::t("native.settings.applyToDraft"),
                ]),
            )
    }
}
