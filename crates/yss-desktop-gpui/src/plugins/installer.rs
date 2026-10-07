//! Signed-package inspection and explicit native execution/signing consent.
use super::{PluginKey, PluginsPanel, failure};
use crate::services::NativeServices;
use gpui::{
    AppContext, Context, IntoElement, PathPromptOptions, Render, WeakEntity, Window, div,
    prelude::*, px,
};
use gpui_component::{
    ActiveTheme, Disableable, WindowExt,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use yss_plugin_runtime::{PackageInspection, operation_id};

struct ApprovedPackage {
    path: PathBuf,
    inspection: PackageInspection,
    operation: String,
    previous_signer: Option<String>,
}

struct PackageInstaller {
    services: Arc<NativeServices>,
    owner: WeakEntity<PluginsPanel>,
    path: PathBuf,
    inspection: Option<PackageInspection>,
    operation: Option<String>,
    approved: bool,
    signer_approved: bool,
    busy: bool,
    error: Option<String>,
}
impl PackageInstaller {
    fn new(
        services: Arc<NativeServices>,
        owner: WeakEntity<PluginsPanel>,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let operation = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|duration| u64::try_from(duration.as_millis()).ok())
            .and_then(|now| operation_id(now, &uuid::Uuid::new_v4().to_string()).ok());
        cx.defer_in(window, |view, window, cx| {
            let path = view.path.clone();
            let job = view
                .services
                .run(move |services| Ok(services.plugins.inspect(&path)));
            cx.spawn_in(window, async move |view, cx| {
                let result = job.await.ok().and_then(Result::ok);
                let _ = view.update(cx, |view, cx| {
                    view.busy = false;
                    match result {
                        Some(Ok(inspection)) => view.inspection = Some(inspection),
                        Some(Err(error)) => view.error = Some(failure(&error)),
                        None => view.error = Some("安装包未读取，请关闭后重新选择。".into()),
                    }
                    cx.notify();
                });
            })
            .detach();
        });
        Self {
            services,
            owner,
            path,
            inspection: None,
            operation,
            approved: false,
            signer_approved: false,
            busy: true,
            error: None,
        }
    }
    fn signer_changed(&self) -> bool {
        self.inspection.as_ref().is_some_and(|inspection| {
            inspection
                .previous_signer_key
                .as_ref()
                .is_some_and(|previous| previous != &inspection.signer_key)
        })
    }
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || !self.approved || (self.signer_changed() && !self.signer_approved) {
            return;
        }
        let Some(inspection) = self.inspection.clone() else {
            return;
        };
        let Some(operation) = self.operation.clone() else {
            self.error = Some("无法创建安装确认，请检查系统时间后重新选择安装包。".into());
            cx.notify();
            return;
        };
        let approved_signer = self
            .signer_changed()
            .then(|| inspection.previous_signer_key.clone())
            .flatten();
        let path = self.path.clone();
        let wizard = cx.entity().downgrade();
        let accepted = self
            .owner
            .update(cx, |view, cx| {
                view.install_package(
                    ApprovedPackage {
                        path,
                        inspection,
                        operation,
                        previous_signer: approved_signer,
                    },
                    wizard,
                    window,
                    cx,
                )
            })
            .unwrap_or(false);
        if accepted {
            self.busy = true;
            self.error = None;
        } else {
            self.error = Some("插件正在处理其他操作，安装确认已保留。".into());
        }
        cx.notify();
    }
}
impl Render for PackageInstaller {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ready = self.inspection.is_some()
            && self.operation.is_some()
            && self.approved
            && (!self.signer_changed() || self.signer_approved)
            && !self.busy;
        let mut content = div().flex().flex_col().gap_3().child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .truncate()
                .child(self.path.display().to_string()),
        );
        if let Some(inspection) = &self.inspection {
            let manifest = &inspection.manifest;
            content = content
                .child(
                    div()
                        .text_lg()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(format!("{} · {}", manifest.name, manifest.version)),
                )
                .child(div().text_sm().child(manifest.description.clone()))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!(
                            "发布者：{} · 目标：{}",
                            manifest.publisher, manifest.target
                        )),
                )
                .child(
                    div()
                        .text_xs()
                        .child(format!("签名指纹：{}", inspection.signer_key)),
                )
                .child(div().text_xs().child(format!(
                    "所需权限：{}",
                    if manifest.permissions.is_empty() {
                        "无声明权限".into()
                    } else {
                        manifest.permissions.join("、")
                    }
                )))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!(
                            "私有存储预算 {:.1} MiB · {} 个并发任务 · {} 个视图",
                            manifest.resource_budget.private_storage_bytes as f64 / 1048576.,
                            manifest.resource_budget.active_tasks,
                            manifest.resource_budget.views
                        )),
                )
                .child(
                    Checkbox::new("plugin-native-consent")
                        .label("我信任此发布者，并允许插件作为当前用户的原生程序运行")
                        .checked(self.approved)
                        .disabled(self.busy)
                        .on_click(cx.listener(|view, checked: &bool, _, cx| {
                            view.approved = *checked;
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("应用预算限制不能隔离原生程序的系统文件访问。"),
                );
            if self.signer_changed() {
                content = content
                    .child(div().text_xs().text_color(cx.theme().danger).child(format!(
                            "签名者已变化。此前指纹：{}",
                            inspection
                                .previous_signer_key
                                .as_deref()
                                .unwrap_or_default()
                        )))
                    .child(
                        Checkbox::new("plugin-signer-consent")
                            .label("确认使用上述新签名者更新此插件")
                            .checked(self.signer_approved)
                            .disabled(self.busy)
                            .on_click(cx.listener(|view, checked: &bool, _, cx| {
                                view.signer_approved = *checked;
                                cx.notify();
                            })),
                    );
            }
        } else if self.busy {
            content = content.child("正在检查签名和内容…");
        }
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                content
                    .id("plugin-install-review")
                    .max_h(px(520.))
                    .overflow_y_scroll(),
            )
            .when_some(self.error.clone(), |view, error| {
                view.child(div().text_sm().text_color(cx.theme().danger).child(error))
            })
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("plugin-install-cancel")
                            .label("取消")
                            .disabled(self.busy)
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("plugin-install-confirm")
                            .primary()
                            .label(if self.busy {
                                "正在处理…"
                            } else {
                                "确认安装"
                            })
                            .disabled(!ready)
                            .on_click(cx.listener(|view, _, window, cx| view.submit(window, cx))),
                    ),
            )
    }
}
impl PluginsPanel {
    pub(super) fn choose_install(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        self.task = Some("正在选择安装包…");
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("选择签名插件包".into()),
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = prompt.await;
            let _ = view.update_in(cx, |view, window, cx| {
                if view.generation != generation {
                    return;
                }
                view.task = None;
                match result {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.into_iter().next() {
                            let owner = cx.entity().downgrade();
                            let wizard = cx.new(|cx| {
                                PackageInstaller::new(
                                    view.services.clone(),
                                    owner,
                                    path,
                                    window,
                                    cx,
                                )
                            });
                            window.open_dialog(cx, move |dialog, _, _| {
                                let cancel = wizard.clone();
                                dialog
                                    .title("检查插件安装包")
                                    .width(px(760.))
                                    .close_button(false)
                                    .overlay_closable(false)
                                    .footer(div())
                                    .child(wizard.clone())
                                    .on_cancel(move |_, _, cx| !cancel.read(cx).busy)
                            });
                        }
                    }
                    Ok(Ok(None)) => {}
                    _ => view.error = Some("安装包选择器未打开，请重试。".into()),
                }
                view.changed(cx);
            });
        })
        .detach();
        self.changed(cx);
    }
    fn install_package(
        &mut self,
        package: ApprovedPackage,
        wizard: WeakEntity<PackageInstaller>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.busy() {
            return false;
        }
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.task = Some("正在安装插件…");
        self.error = None;
        self.feedback = None;
        let job = self.services.run(move |services| {
            let result = services.plugins.install(
                &package.path,
                &package.inspection.package_digest,
                &package.operation,
                true,
                package.previous_signer.as_deref(),
            );
            Ok((result, services.plugins.list()))
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.generation != generation {
                    return;
                }
                view.task = None;
                let (installed, error) = if let Some((result, entries)) = result {
                    match entries {
                        Ok(entries) => view.install_entries(entries),
                        Err(error) => view.error = Some(failure(&error)),
                    }
                    match result {
                        Ok(plugin) => {
                            view.selected = Some(PluginKey::from_plugin(&plugin));
                            (true, None)
                        }
                        Err(error) => (false, Some(failure(&error))),
                    }
                } else {
                    (
                        false,
                        Some("安装结果未确认，请在此窗口重试；已检查的安装包会保留。".into()),
                    )
                };
                if installed {
                    view.feedback = Some("插件已安装。".into());
                    window.close_dialog(cx);
                } else if let Some(error) = error {
                    view.error = Some(error.clone());
                    let _ = wizard.update(cx, |wizard, cx| {
                        wizard.busy = false;
                        wizard.error = Some(error);
                        cx.notify();
                    });
                }
                view.cursor_stack = vec![None];
                if view.reload_again {
                    view.reload_again = false;
                    view.reload(window, cx);
                } else if let Some(key) = view.selected.clone() {
                    view.read_details(key, None, window, cx);
                }
                view.changed(cx);
            });
        })
        .detach();
        self.changed(cx);
        true
    }
}
