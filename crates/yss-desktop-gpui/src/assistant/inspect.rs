//! Tool details consume the existing sanitized ledger projection.
use super::ConversationPanel;
use gpui::{ClipboardItem, Context, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Sizable, WindowExt,
    button::{Button, ButtonVariants},
};
use yss_harness_contract::{AssistantToolInspection, ToolInvocationId};

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
                        window.open_dialog(cx, move |dialog, window, cx| {
                            window.use_keyed_state("resource-observer", cx, |window, cx| {
                                owner.upgrade().map(|owner| {
                                    cx.observe_in(&owner, window, |_, _, window, _| {
                                        window.refresh()
                                    })
                                })
                            });
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
                            body = body.child(format!(
                                "开始：{}",
                                super::execution::timestamp(detail.started_at)
                            ));
                            if let Some(finished) = detail.finished_at {
                                body = body.child(format!(
                                    "结束：{}",
                                    super::execution::timestamp(finished)
                                ));
                            }
                            if let Some(failure) = &detail.failure {
                                body = body.child(
                                    div()
                                        .text_color(cx.theme().danger)
                                        .child(format!("失败：{failure}")),
                                );
                            }
                            let catalog = owner
                                .upgrade()
                                .and_then(|owner| owner.read(cx).resource_catalog.clone());
                            body = body.child(super::resources::cards(
                                "tool-artifacts",
                                &owner,
                                &detail.artifacts,
                                &detail.results,
                                catalog.as_deref(),
                                cx,
                            ));
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
}
