//! Composer controls use the original model catalog, options and project resource identities.
use super::ConversationPanel;
use gpui::{AnyElement, Context, IntoElement, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::{Input, Textarea},
};
use yss_project_identity::{ProjectResourceKind as Kind, ProjectResourceRef};

impl ConversationPanel {
    pub(super) fn load_resources(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.resource_generation = self.resource_generation.wrapping_add(1);
        let generation = self.resource_generation;
        let project = self.session.project.project_instance_id().clone();
        let job = self.services.run(move |services| {
            Ok(services
                .application
                .query_project_index(project, "zh-CN", false)?
                .index)
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            let _ = view.update_in(cx, |view, _, cx| {
                if view.resource_generation != generation {
                    return;
                }
                if let Some(index) = result {
                    let mut choices = vec![];
                    let mut add = |kind, id: String, name: String| {
                        choices.push(yss_harness_contract::HarnessResourceReference {
                            resource: ProjectResourceRef { kind, id },
                            name,
                        })
                    };
                    for entry in index.databases {
                        add(
                            Kind::Database,
                            entry.id.clone(),
                            entry.name.unwrap_or(entry.id),
                        );
                    }
                    for entry in index.event_graphs {
                        add(Kind::EventGraph, entry.path, entry.name);
                    }
                    for entry in index.function_graphs {
                        add(Kind::FunctionGraph, entry.path, entry.name);
                    }
                    for entry in index.charts {
                        add(
                            Kind::Chart,
                            entry.chart_path.as_str().to_owned(),
                            entry.name,
                        );
                    }
                    for entry in index.docs {
                        add(Kind::Doc, entry.path.as_str().to_owned(), entry.name);
                    }
                    for entry in index.minds {
                        add(Kind::Mind, entry.path.as_str().to_owned(), entry.name);
                    }
                    view.resource_choices = choices;
                } else {
                    view.resource_choices.clear();
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn render_composer(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut composer = div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .flex_shrink_0()
            .border_t_1()
            .border_color(cx.theme().border);
        if let Some(message) = &self.unsent {
            composer = composer.child(
                div()
                    .p_2()
                    .rounded_md()
                    .bg(cx.theme().muted)
                    .text_xs()
                    .child("未确认的原文已保留，请检查历史后再决定是否重发。")
                    .child(
                        div()
                            .max_h(px(65.))
                            .overflow_hidden()
                            .child(message.text.clone()),
                    )
                    .child(
                        Button::new("assistant-restore")
                            .small()
                            .ghost()
                            .label("恢复到输入区")
                            .on_click(
                                cx.listener(|view, _, window, cx| view.restore_unsent(window, cx)),
                            ),
                    ),
            );
        }
        if !self.queue.is_empty() {
            let mut queued = div()
                .flex()
                .flex_col()
                .gap_1()
                .p_2()
                .rounded_md()
                .bg(cx.theme().muted);
            for message in &self.queue {
                let id = message.id;
                queued = queued.child(
                    div()
                        .flex()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .text_xs()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(message.text.clone()),
                        )
                        .child(
                            Button::new(gpui::SharedString::from(format!("queued-{id}")))
                                .small()
                                .ghost()
                                .label("移除")
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.queue.retain(|message| message.id != id);
                                    cx.notify();
                                })),
                        ),
                );
            }
            composer = composer.child(
                queued.child(
                    Button::new("assistant-send-queued")
                        .small()
                        .ghost()
                        .label("发送下一条")
                        .disabled(!self.can_send())
                        .on_click(cx.listener(|view, _, window, cx| view.send_queued(window, cx))),
                ),
            );
        }
        let mut references = div().flex().flex_wrap().gap_1();
        for (index, resource) in self.references.iter().enumerate() {
            let label = self
                .resource_choices
                .iter()
                .find(|choice| choice.resource == *resource)
                .map(|choice| choice.name.clone())
                .unwrap_or_else(|| resource.id.clone());
            let resource = resource.clone();
            references = references.child(
                Button::new(("assistant-reference", index))
                    .small()
                    .ghost()
                    .label(format!("{label} ×"))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.references.retain(|candidate| candidate != &resource);
                        cx.notify();
                    })),
            );
        }
        composer = composer.child(references);
        if self.reference_picker {
            composer = composer.child(self.render_resource_picker(cx));
        }
        composer
            .child(Textarea::new(&self.input).bordered(false).appearance(false))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_1()
                    .child(self.usage_indicator(cx))
                    .child(self.model_picker(window, cx))
                    .child(self.mode_picker(cx))
                    .child(self.effort_picker(cx))
                    .child(
                        Button::new("assistant-pick-resource")
                            .small()
                            .ghost()
                            .label("引用资源")
                            .disabled(!self.ready)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.reference_picker = !view.reference_picker;
                                if view.reference_picker {
                                    view.load_resources(window, cx);
                                }
                                cx.notify();
                            })),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("assistant-queue")
                            .small()
                            .ghost()
                            .label("加入队列")
                            .disabled(
                                !self.ready
                                    || self.selecting
                                    || self.input.read(cx).value().trim().is_empty(),
                            )
                            .on_click(
                                cx.listener(|view, _, window, cx| view.queue_message(window, cx)),
                            ),
                    )
                    .child(if self.running() {
                        Button::new("assistant-stop")
                            .small()
                            .danger()
                            .label(if self.stopping {
                                "正在停止"
                            } else {
                                "停止"
                            })
                            .disabled(self.stopping)
                            .on_click(cx.listener(|view, _, _, cx| view.cancel(cx)))
                    } else {
                        Button::new("assistant-send")
                            .small()
                            .primary()
                            .label("发送")
                            .disabled(
                                !self.can_send() || self.input.read(cx).value().trim().is_empty(),
                            )
                            .on_click(cx.listener(|view, _, window, cx| view.send(window, cx)))
                    }),
            )
            .into_any_element()
    }
    fn render_resource_picker(&self, cx: &mut Context<Self>) -> AnyElement {
        let query = self.resource_search.read(cx).value().to_lowercase();
        let mut choices = div().flex().flex_col().gap_1();
        for (index, choice) in self
            .resource_choices
            .iter()
            .enumerate()
            .filter(|(_, choice)| {
                choice.name.to_lowercase().contains(&query)
                    || choice.resource.id.to_lowercase().contains(&query)
            })
        {
            let resource = choice.resource.clone();
            let selected = self.references.contains(&resource);
            choices = choices.child(
                Button::new(("assistant-resource-option", index))
                    .small()
                    .ghost()
                    .label(format!("{} · {}", choice.name, choice.resource.id))
                    .toggled(selected)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if !view.references.contains(&resource) {
                            view.references.push(resource.clone());
                        }
                        view.reference_picker = false;
                        cx.notify();
                    })),
            );
        }
        div()
            .p_2()
            .rounded_md()
            .bg(cx.theme().muted)
            .flex()
            .flex_col()
            .gap_2()
            .child(Input::new(&self.resource_search).small())
            .child(
                div()
                    .id("assistant-resource-choices")
                    .max_h(px(180.))
                    .overflow_y_scroll()
                    .child(choices),
            )
            .into_any_element()
    }
}
