use super::SettingsPanel;
use gpui::{AnyElement, Context, IntoElement, div, prelude::*};
use gpui_component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::{Input, Textarea},
};
use yss_harness_contract::ReasoningEffort;

impl SettingsPanel {
    pub(in crate::settings) fn provider_form(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(draft) = &self.editor else {
            return div().into_any_element();
        };
        let busy = self.busy();
        let mut content = div().flex().flex_col().child(self.connection_fields(cx));
        content = content
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .pt_5()
                    .pb_3()
                    .child(div().text_sm().flex_1().child("模型"))
                    .child(
                        Button::new("models-discover")
                            .small()
                            .ghost()
                            .label(crate::text::translate("settings.models.discover"))
                            .disabled(busy || !self.connection_ready(cx))
                            .on_click(cx.listener(|view, _, window, cx| view.discover(window, cx))),
                    )
                    .child(
                        Button::new("models-add")
                            .small()
                            .ghost()
                            .label("添加模型")
                            .disabled(busy || self.model.is_some())
                            .on_click(
                                cx.listener(|view, _, window, cx| {
                                    view.edit_model(None, window, cx)
                                }),
                            ),
                    ),
            )
            .child(self.model_list(cx));
        if self.model.is_some() {
            content = content.child(self.model_form(cx));
        }
        if draft.saved {
            content = content.child(
                div().pt_6().child(
                    Button::new("provider-delete")
                        .small()
                        .danger()
                        .label(crate::text::translate("settings.models.removeProvider"))
                        .disabled(busy)
                        .on_click(
                            cx.listener(|view, _, window, cx| view.delete_provider(window, cx)),
                        ),
                ),
            );
        }
        content.into_any_element()
    }

    fn model_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let draft = self.editor.as_ref().unwrap();
        let mut content = div().flex().flex_col().gap_2();
        if draft.models.is_empty() {
            content = content.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("尚未添加模型。"),
            );
        }
        for (index, model) in draft.models.iter().enumerate() {
            let epoch = self.epoch;
            let edit_id = model.id.clone();
            let remove_id = model.id.clone();
            content = content.child(
                div()
                    .p_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_sm().child(model.name.clone()))
                            .child(
                                div()
                                    .text_xs()
                                    .truncate()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(model.id.clone()),
                            ),
                    )
                    .child(
                        Button::new(("model-edit", index))
                            .small()
                            .ghost()
                            .label("编辑")
                            .disabled(self.busy())
                            .on_click(cx.listener(move |view, _, window, cx| {
                                if view.model_row_current(epoch, index, &edit_id) {
                                    view.edit_model(Some(index), window, cx);
                                }
                            })),
                    )
                    .child(
                        Button::new(("model-remove", index))
                            .small()
                            .ghost()
                            .label("移除")
                            .disabled(self.busy() || self.model.is_some())
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
                    ),
            );
        }
        content.into_any_element()
    }

    fn model_form(&self, cx: &mut Context<Self>) -> AnyElement {
        let model = self.model.as_ref().unwrap();
        let busy = self.busy();
        let mut form = div()
            .mt_4()
            .p_4()
            .rounded_lg()
            .bg(cx.theme().muted)
            .child(self.render_field(
                "模型 ID",
                "服务端使用的精确模型标识。",
                Input::new(&model.id).disabled(busy),
                cx,
            ))
            .child(self.render_field(
                "显示名称",
                "模型选择器中的名称。",
                Input::new(&model.name).disabled(busy),
                cx,
            ))
            .child(self.render_field(
                "上下文容量",
                "可选；Token 数。",
                Input::new(&model.context).disabled(busy),
                cx,
            ))
            .child(self.render_field(
                "最大输出",
                "可选；Anthropic 协议必填。",
                Input::new(&model.output).disabled(busy),
                cx,
            ))
            .child(self.render_field(
                "Temperature",
                "留空继承服务默认值。",
                Input::new(&model.temperature).disabled(busy),
                cx,
            ))
            .child(self.render_field(
                "Top P",
                "留空继承服务默认值。",
                Input::new(&model.top_p).disabled(busy),
                cx,
            ));
        let mut efforts = div().flex().gap_2().flex_wrap();
        for (index, (label, effort)) in [
            ("Low", ReasoningEffort::Low),
            ("Medium", ReasoningEffort::Medium),
            ("High", ReasoningEffort::High),
        ]
        .into_iter()
        .enumerate()
        {
            efforts = efforts.child(
                Button::new(("model-effort", index))
                    .small()
                    .ghost()
                    .label(label)
                    .toggled(model.efforts.contains(&effort))
                    .disabled(busy)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if view.busy() {
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
        form = form
            .child(self.render_field(
                "推理档位限制",
                "全部留空时由服务验证所选档位。",
                efforts,
                cx,
            ))
            .child(self.render_field(
                "扩展参数",
                "JSON 生成参数；不能替换消息、工具和认证。",
                Textarea::new(&model.parameters).disabled(busy),
                cx,
            ))
            .child(
                div()
                    .pt_4()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("model-cancel")
                            .small()
                            .ghost()
                            .label("取消模型编辑")
                            .disabled(busy)
                            .on_click(cx.listener(|view, _, _, cx| {
                                if view.busy() {
                                    return;
                                }
                                view.model = None;
                                view.model_subscriptions.clear();
                                view.error = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("model-apply")
                            .small()
                            .primary()
                            .label("应用到草稿")
                            .disabled(busy)
                            .on_click(cx.listener(|view, _, _, cx| view.apply_model(cx))),
                    ),
            );
        form.into_any_element()
    }
}
