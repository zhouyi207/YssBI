//! Tool/citation details use the existing sanitized ledger and knowledge projections.
use super::{ConversationEvent, ConversationPanel};
use gpui::{ClipboardItem, Context, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Disableable, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    text::TextView,
};
use yss_harness_contract::{KnowledgeCitation, ToolInvocationId};
use yss_harness_contract::{AssistantResultReference, AssistantToolInspection};

impl ConversationPanel {
    pub(super) fn inspect_tool(&self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Ok(invocation) = ToolInvocationId::try_new(id) else {
            return;
        };
        let services = self.services.clone();
        let principal = self.principal.clone();
        let session = self.session.id.clone();
        let generation = self.generation;
        let job = self.services.executor.spawn(async move {
            services
                .application
                .application
                .validate_harness_session(&services.application.harness.host, &principal, &session)
                .await
                .map_err(super::commands::session_failure)?;
            let record = services
                .application
                .harness
                .host
                .inspect_tool_invocation(&session, &invocation)
                .await
                .map_err(super::commands::harness_failure)?;
            let detail = if let Some(record) = record {
                Some(AssistantToolInspection::from(record))
            } else {
                let events = services
                    .application
                    .harness
                    .host
                    .events_after(&session, 0)
                    .await
                    .map_err(super::commands::harness_failure)?;
                AssistantToolInspection::from_control_events(&events, &invocation)
            };
            services
                .application
                .application
                .validate_harness_session(&services.application.harness.host, &principal, &session)
                .await
                .map_err(super::commands::session_failure)?;
            detail.ok_or_else(|| "工具详情已不可用。".to_owned())
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job
                .await
                .unwrap_or_else(|_| Err("工具详情读取失败。".into()));
            let _ = view.update_in(cx, |view, window, cx| {
                if view.generation != generation {
                    return;
                }
                match result {
                    Ok(detail) => {
                        let owner = cx.entity().downgrade();
                        window.open_dialog(cx, move |dialog, _, cx| {
                            let mut body = div()
                                .id("tool-detail")
                                .max_h(px(460.))
                                .overflow_y_scroll()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .text_sm();
                            if let Some(target) = &detail.target {
                                body = body.child(format!("目标：{target}"));
                            }
                            for (name, value) in &detail.parameters {
                                body = body.child(format!("{name}：{value}"));
                            }
                            body = body.child(format!("开始：{}", time(detail.started_at)));
                            if let Some(finished) = detail.finished_at {
                                body = body.child(format!("结束：{}", time(finished)));
                            }
                            if let Some(failure) = &detail.failure {
                                body = body.child(
                                    div()
                                        .text_color(cx.theme().danger)
                                        .child(format!("失败：{failure}")),
                                );
                            }
                            for (index, artifact) in detail.artifacts.iter().enumerate() {
                                let resource = artifact.resource.clone();
                                let owner = owner.clone();
                                body = body.child(
                                    Button::new(("tool-artifact", index))
                                        .small()
                                        .ghost()
                                        .label(resource.id.clone())
                                        .disabled(artifact.deleted)
                                        .on_click(move |_, _, cx| {
                                            let _ = owner.update(cx, |_, cx| {
                                                cx.emit(ConversationEvent::OpenResource(
                                                    resource.clone(),
                                                ))
                                            });
                                        }),
                                );
                            }
                            for (index, result) in detail.results.iter().enumerate() {
                                let result = result.clone();
                                let owner = owner.clone();
                                body = body.child(
                                    Button::new(("tool-result", index))
                                        .small()
                                        .ghost()
                                        .label(result.output.clone())
                                        .on_click(move |_, _, cx| {
                                            let _ = owner.update(cx, |view, cx| {
                                                view.open_result(&result, cx)
                                            });
                                        }),
                                );
                            }
                            let copy = serde_json::to_string_pretty(&detail).unwrap_or_default();
                            dialog.title("工具详情").width(px(620.)).child(body).footer(
                                div().flex().justify_end().child(
                                    Button::new("copy-tool-detail")
                                        .small()
                                        .ghost()
                                        .label("复制详情")
                                        .on_click(move |_, _, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                copy.clone(),
                                            ))
                                        }),
                                ),
                            )
                        });
                    }
                    Err(error) => {
                        view.error = Some(error);
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }
    pub(super) fn inspect_citation(
        &self,
        citation: KnowledgeCitation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let services = self.services.clone();
        let session = self.session.id.clone();
        let principal = self.principal.clone();
        let generation = self.generation;
        let title = citation.title.clone();
        let job = self.services.executor.spawn(async move {
            services
                .application
                .application
                .validate_harness_session(&services.application.harness.host, &principal, &session)
                .await
                .map_err(super::commands::session_failure)?;
            let text = services
                .application
                .harness
                .host
                .inspect_citation(&session, &citation)
                .await
                .map_err(super::commands::harness_failure)?
                .ok_or_else(|| "引用来源已变化或不可用。".to_owned())?;
            let resource = services
                .application
                .harness
                .knowledge
                .citation_resource(&citation)
                .await
                .map_err(|_| "引用来源不可用。".to_owned())?;
            services
                .application
                .application
                .validate_harness_session(&services.application.harness.host, &principal, &session)
                .await
                .map_err(super::commands::session_failure)?;
            Ok::<_, String>((text, resource))
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.unwrap_or_else(|_| Err("引用读取未完成。".into()));
            let _ = view.update_in(cx, |view, window, cx| {
                if view.generation != generation {
                    return;
                }
                match result {
                    Ok((text, resource)) => {
                        let owner = cx.entity().downgrade();
                        window.open_dialog(cx, move |dialog, _, _| {
                            dialog
                                .title(title.clone())
                                .width(px(700.))
                                .child(
                                    div()
                                        .id("citation-text")
                                        .max_h(px(440.))
                                        .overflow_y_scroll()
                                        .child(TextView::markdown("citation", text.clone())),
                                )
                                .footer(div().flex().justify_end().when_some(
                                    resource.clone(),
                                    |view, resource| {
                                        let owner = owner.clone();
                                        view.child(
                                            Button::new("citation-open-source")
                                                .small()
                                                .ghost()
                                                .label("打开来源")
                                                .on_click(move |_, _, cx| {
                                                    let _ = owner.update(cx, |_, cx| {
                                                        cx.emit(ConversationEvent::OpenResource(
                                                            resource.clone(),
                                                        ))
                                                    });
                                                }),
                                        )
                                    },
                                ))
                        });
                    }
                    Err(error) => {
                        view.error = Some(error);
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }
    pub(super) fn open_result(&self, result: &AssistantResultReference, cx: &mut Context<Self>) {
        if let (Ok(session), Ok(id)) = (
            result.execution_session_id.parse(),
            result.result_id.parse(),
        ) {
            cx.emit(ConversationEvent::OpenResult(
                yss_graph_execution::result::ResultReference {
                    execution_session_id: yss_graph_execution::identity::ExecutionSessionId::new(
                        session,
                    ),
                    result_id: yss_graph_execution::result::ResultId::from_existing(id),
                },
            ));
        }
    }
}
fn time(milliseconds: u64) -> String {
    i64::try_from(milliseconds)
        .ok()
        .and_then(chrono::DateTime::from_timestamp_millis)
        .map(|time| time.naive_utc().format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "时间不可用".into())
}
