//! Signed-package inspection and explicit native execution/signing consent.
use super::{PluginKey, PluginsPanel, failure};
use crate::PluginServices;
use gpui_kit::component::{
    ActiveTheme, Disableable, WindowExt,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    notification::Notification,
};
use gpui_kit::{
    AppContext, Context, IntoElement, PathPromptOptions, Render, WeakEntity, Window, div,
    prelude::*, px,
};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use yss_plugin_runtime::{PackageInspection, operation_id};
use yss_ui::lifecycle::LogReleased;

struct ApprovedPackage {
    path: PathBuf,
    inspection: PackageInspection,
    operation: String,
    previous_signer: Option<String>,
}

struct PackageInstaller {
    services: Arc<PluginServices>,
    owner: WeakEntity<PluginsPanel>,
    path: PathBuf,
    inspection: Option<PackageInspection>,
    operation: Option<String>,
    approved: bool,
    signer_approved: bool,
    busy: bool,
    error: Option<String>,
    notifications: yss_ui::notifications::Notifications,
}
impl PackageInstaller {
    fn set_error(&mut self, error: Option<String>, cx: &mut gpui_kit::Context<Self>) {
        self.notifications.replace_error(&mut self.error, error, cx);
    }

    fn new(
        services: Arc<PluginServices>,
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
                view.update(cx, |view, cx| {
                    view.busy = false;
                    match result {
                        Some(Ok(inspection)) => view.inspection = Some(inspection),
                        Some(Err(error)) => view.set_error(Some(failure(&error)), cx),
                        None => view.set_error(
                            Some(yss_ui::text::t("native.plugins.packageReadFailed").into()),
                            cx,
                        ),
                    }
                    cx.notify();
                })
                .log_released();
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
            notifications: yss_ui::notifications::Notifications::new(window),
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
            self.set_error(
                Some(yss_ui::text::t("native.plugins.confirmationFailed").into()),
                cx,
            );
            cx.notify();
            return;
        };
        let approved_signer = self
            .signer_changed()
            .then(|| inspection.previous_signer_key.clone())
            .flatten();
        let path = self.path.clone();
        let wizard = cx.entity().downgrade();
        let failure = wizard.clone();
        let owner = self.owner.clone();
        let parent = yss_ui::modal_window::owner_window(window, cx);
        self.busy = true;
        self.error = None;
        cx.defer(move |cx| {
            let accepted = parent
                .update(cx, |_, window, cx| {
                    owner
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
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            if !accepted {
                failure
                    .update(cx, |wizard, cx| {
                        wizard.busy = false;
                        wizard.set_error(
                            Some(yss_ui::text::t("native.plugins.installationBusy").into()),
                            cx,
                        );
                        cx.notify();
                    })
                    .log_released();
            }
        });
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
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .child(format!("{} · {}", manifest.name, manifest.version)),
                )
                .child(div().text_sm().child(manifest.description.clone()))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(yss_ui::text::format(
                            "native.plugins.publisherAndTarget",
                            &[
                                ("value0", manifest.publisher.to_string()),
                                ("value1", manifest.target.to_string()),
                            ],
                        )),
                )
                .child(div().text_xs().child(yss_ui::text::format(
                    "native.plugins.signatureFingerprint",
                    &[("value0", inspection.signer_key.to_string())],
                )))
                .child(
                    div().text_xs().child(yss_ui::text::format(
                        "native.plugins.requiredPermissions",
                        &[(
                            "value0",
                            (if manifest.permissions.is_empty() {
                                yss_ui::text::t("native.plugins.noPermissions").into()
                            } else {
                                manifest
                                    .permissions
                                    .join(&yss_ui::text::t("common.listSeparator"))
                            })
                            .to_string(),
                        )],
                    )),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(yss_ui::text::format(
                            "native.plugins.budgetSummary",
                            &[
                                (
                                    "value0",
                                    format!(
                                        "{:.1}",
                                        manifest.resource_budget.private_storage_bytes as f64
                                            / 1048576.
                                    ),
                                ),
                                ("value1", manifest.resource_budget.active_tasks.to_string()),
                                ("value2", manifest.resource_budget.views.to_string()),
                            ],
                        )),
                )
                .child(
                    Checkbox::new("plugin-native-consent")
                        .label(gpui_kit::SharedString::from(yss_ui::text::t(
                            "native.plugins.trustPublisher",
                        )))
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
                        .child(yss_ui::text::t("native.plugins.budgetWarning")),
                );
            if self.signer_changed() {
                content = content
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().danger)
                            .child(yss_ui::text::format(
                                "native.plugins.previousSigner",
                                &[(
                                    "value0",
                                    inspection
                                        .previous_signer_key
                                        .as_deref()
                                        .unwrap_or_default()
                                        .to_string(),
                                )],
                            )),
                    )
                    .child(
                        Checkbox::new("plugin-signer-consent")
                            .label(gpui_kit::SharedString::from(yss_ui::text::t(
                                "native.plugins.confirmNewSigner",
                            )))
                            .checked(self.signer_approved)
                            .disabled(self.busy)
                            .on_click(cx.listener(|view, checked: &bool, _, cx| {
                                view.signer_approved = *checked;
                                cx.notify();
                            })),
                    );
            }
        } else if self.busy {
            content = content.child(yss_ui::text::t("native.plugins.verifying"));
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
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("plugin-install-cancel")
                            .label(yss_ui::text::t("common.cancel"))
                            .disabled(self.busy)
                            .on_click(|_, window, cx| yss_ui::modal_window::close(window, cx)),
                    )
                    .child(
                        Button::new("plugin-install-confirm")
                            .primary()
                            .label(if self.busy {
                                yss_ui::text::t("native.assistant.processing")
                            } else {
                                yss_ui::text::t("native.plugins.confirmInstall")
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
        self.busy = true;
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(yss_ui::text::t("native.plugins.chooseSignedPackage").into()),
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = prompt.await;
            view.update_in(cx, |view, window, cx| {
                if view.generation != generation {
                    return;
                }
                view.busy = false;
                match result {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.into_iter().next() {
                            let owner = cx.entity().downgrade();
                            let services = view.services.clone();
                            yss_ui::modal_window::open(
                                yss_ui::text::t("native.plugins.inspectPackage"),
                                gpui_kit::size(px(800.), px(740.)),
                                window,
                                cx,
                                move |window, cx| {
                                    let wizard = cx.new(|cx| {
                                        PackageInstaller::new(services, owner, path, window, cx)
                                    });
                                    let cancel = wizard.clone();
                                    yss_ui::modal_window::ModalContent::new(move |_, _| {
                                        wizard.clone()
                                    })
                                    .without_buttons()
                                    .on_cancel(move |_, _, cx| !cancel.read(cx).busy)
                                },
                            );
                        }
                    }
                    Ok(Ok(None)) => {}
                    _ => window.push_notification(
                        Notification::error(yss_ui::text::t("native.plugins.packagePickerFailed")),
                        cx,
                    ),
                }
                view.changed(cx);
            })
            .log_released();
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
        self.busy = true;
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
            view.update_in(cx, |view, window, cx| {
                if view.generation != generation {
                    return;
                }
                view.busy = false;
                let (installed, error) = if let Some((result, entries)) = result {
                    match entries {
                        Ok(entries) => view.install_entries(entries),
                        Err(error) => {
                            window.push_notification(Notification::error(failure(&error)), cx);
                        }
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
                        Some(yss_ui::text::t("native.plugins.installationUnconfirmed").into()),
                    )
                };
                if installed {
                    yss_ui::modal_window::close_child(window, cx);
                } else if let Some(error) = error {
                    wizard
                        .update(cx, |wizard, cx| {
                            wizard.busy = false;
                            wizard.set_error(Some(error), cx);
                            cx.notify();
                        })
                        .log_released();
                }
                view.cursor_stack = vec![None];
                if view.reload_again {
                    view.reload_again = false;
                    view.reload(window, cx);
                } else if let Some(key) = view.selected.clone() {
                    view.read_details(key, None, window, cx);
                }
                view.changed(cx);
            })
            .log_released();
        })
        .detach();
        self.changed(cx);
        true
    }
}
