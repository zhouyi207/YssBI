//! The first window owns service initialization until the workbench can take over.
use crate::{services::NativeServices, text, window_chrome, workbench::Workbench};
use gpui_kit::component::{
    ActiveTheme, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    notification::Notification,
    progress::Progress,
};
use gpui_kit::{
    AppContext, Context, Entity, IntoElement, Render, Task, Window, div, prelude::*, px,
};
use std::path::PathBuf;
use tokio::{runtime::Handle, task::AbortHandle};
use yss_application::runtime::ApplicationStartupError;

enum State {
    Loading {
        worker: AbortHandle,
        _delivery: Task<()>,
    },
    Failed(anyhow::Error),
    Ready(Entity<Workbench>),
}

pub(crate) struct Startup {
    state: State,
}

impl Startup {
    pub(crate) fn new(
        executor: Handle,
        project: Option<PathBuf>,
        resource: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let runtime = executor.clone();
        let job = executor.spawn(async move { NativeServices::initialize(runtime).await });
        let worker = job.abort_handle();
        let delivery = cx.spawn_in(window, async move |view, cx| {
            let result = job
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            match result {
                Ok(services) => {
                    let restore_last = view.update_in(cx, |_, window, cx| {
                        if services.preference_load_failed {
                            window.push_notification(
                                Notification::error(text::translate(
                                    "preferences.startup.loadFailed",
                                ))
                                .autohide(false),
                                cx,
                            );
                        }
                        if let Err(error) =
                            crate::preferences::publish(services.preferences.snapshot(), cx)
                        {
                            tracing::warn!(
                                code = "native_preferences_publish_failed",
                                %error,
                                "Native preferences could not be applied"
                            );
                            // Invalid overrides must not prevent opening the workbench.
                            // Keep the saved file untouched so Settings can repair it.
                            if let Err(error) = crate::preferences::publish(
                                std::sync::Arc::new(yss_settings::UserSettings::default()),
                                cx,
                            ) {
                                tracing::error!(%error, "Default preferences could not be applied");
                            }
                            window.push_notification(
                                Notification::error(text::translate(
                                    "preferences.startup.applyFailed",
                                ))
                                .autohide(false),
                                cx,
                            );
                        }
                        project.is_none()
                            && resource.is_none()
                            && crate::preferences::current(cx)
                                .workspace
                                .restore_last_project
                    });
                    let mut project = project;
                    let mut recent_failed = false;
                    if matches!(restore_last, Ok(true)) {
                        let owner = services.clone();
                        let job = services
                            .executor
                            .spawn_blocking(move || owner.layouts.last_project());
                        match job.await {
                            Ok(Ok(last_project)) => project = last_project,
                            _ => recent_failed = true,
                        }
                    }
                    let _ = view.update_in(cx, |view, window, cx| {
                        if recent_failed {
                            window.push_notification(
                                Notification::error(text::translate(
                                    "preferences.startup.recentFailed",
                                ))
                                .autohide(false),
                                cx,
                            );
                        }
                        // Resolve this once at startup. A later explicit Close must stay closed.
                        view.state = State::Ready(
                            cx.new(|cx| Workbench::new(services, project, resource, window, cx)),
                        );
                        cx.notify();
                    });
                }
                Err(error) => {
                    let _ = view.update_in(cx, |view, _, cx| {
                        view.state = State::Failed(error);
                        cx.notify();
                    });
                }
            }
        });
        Self {
            state: State::Loading {
                worker,
                _delivery: delivery,
            },
        }
    }

    pub(crate) fn close_requested(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        match &self.state {
            State::Ready(workbench) => {
                workbench.update(cx, |view, cx| view.close_requested(window, cx))
            }
            _ => true,
        }
    }
}

impl Drop for Startup {
    fn drop(&mut self) {
        if let State::Loading { worker, .. } = &self.state {
            // Dropping a Tokio JoinHandle only detaches it. Window closure must
            // also stop pending initialization, not just its UI delivery.
            worker.abort();
        }
    }
}

impl Render for Startup {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let State::Ready(workbench) = &self.state {
            return workbench.clone().into_any_element();
        }
        let failed = matches!(self.state, State::Failed(_));
        let label = text::translate(if failed {
            "native.startup.failed"
        } else {
            "native.startup.initializing"
        });
        let mut content = div()
            .id("startup-status")
            .role(if failed {
                gpui_kit::accesskit::Role::Alert
            } else {
                gpui_kit::accesskit::Role::Status
            })
            .aria_label(label.clone())
            .w_full()
            .max_w(px(420.))
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_sm().child(label.clone()));
        if let State::Failed(error) = &self.state {
            content = content.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(text::translate(failure_key(error))),
            );
            if let Some(failure) = error
                .chain()
                .find_map(|cause| cause.downcast_ref::<yss_harness_contract::PersistenceFailure>())
            {
                content = content.child(div().text_xs().child(text::format(
                    "native.startup.failureCode",
                    &[("code", failure.code.to_string())],
                )));
            }
            content = content.child(
                Button::new("close-startup")
                    .primary()
                    .label(text::translate("native.startup.close"))
                    .on_click(|_, window, _| window.remove_window()),
            );
        } else {
            content = content.child(
                Progress::new("startup-progress")
                    .small()
                    .loading(true)
                    .accessibility_label(label),
            );
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(window_chrome::title_bar(
                |_, window, _| window.remove_window(),
                div().text_sm().child("YssBI"),
                window,
                cx,
            ))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .p_6()
                    .child(content),
            )
            .into_any_element()
    }
}

fn failure_key(error: &anyhow::Error) -> &'static str {
    match error.downcast_ref::<ApplicationStartupError>() {
        Some(ApplicationStartupError::Application(_)) => "native.startup.applicationFailed",
        Some(ApplicationStartupError::Harness(_)) => "native.startup.harnessFailed",
        Some(ApplicationStartupError::ProjectRegistry(_)) => "native.startup.registryFailed",
        None => "native.startup.environmentFailed",
    }
}
