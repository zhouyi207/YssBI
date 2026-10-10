//! Destination input is a draft; the existing Project use case validates and commits it.
use super::progress::ProjectProgress;
use crate::{
    services::NativeServices,
    workbench::{
        Workbench,
        projects::{ProjectCommand, ProjectOperation},
    },
};
use gpui_kit::component::{
    ActiveTheme, Disableable,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
};
use gpui_kit::{
    AppContext, Context, Entity, IntoElement, PathPromptOptions, Render, Subscription, WeakEntity,
    Window, div, prelude::*,
};
use std::{
    path::{Component, Path, PathBuf},
    sync::Arc,
};

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ProjectFormKind {
    Create,
    SaveAs,
}
impl ProjectFormKind {
    pub(crate) fn title(self) -> &'static str {
        if self == Self::Create {
            crate::text::t("projectPicker.newProjectModal.title")
        } else {
            crate::text::t("native.projects.saveAs")
        }
    }
}
pub(crate) struct ProjectForm {
    services: Arc<NativeServices>,
    owner: WeakEntity<Workbench>,
    pub(crate) lifecycle: u64,
    kind: ProjectFormKind,
    name: Entity<InputState>,
    parent: Entity<InputState>,
    original_name: String,
    original_parent: String,
    default_parent_pending: bool,
    pub(crate) busy: bool,
    progress: Option<Entity<ProjectProgress>>,
    error: Option<String>,
    recovery: Option<PathBuf>,
    confirming: bool,
    _subscriptions: Vec<Subscription>,
}
impl ProjectForm {
    pub(crate) fn new(
        services: Arc<NativeServices>,
        owner: WeakEntity<Workbench>,
        lifecycle: u64,
        kind: ProjectFormKind,
        default_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).default_value(default_name.clone()));
        let parent = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(crate::text::t("native.projects.parentPlaceholder"))
        });
        let mut subscriptions: Vec<_> = [&name, &parent]
            .into_iter()
            .map(|input| cx.observe(input, |_, _, cx| cx.notify()))
            .collect();
        subscriptions.push(cx.subscribe(&parent, |view, _, event, _| {
            if matches!(event, InputEvent::Change) {
                view.default_parent_pending = false;
            }
        }));
        cx.defer_in(window, |view, window, cx| {
            let job = view
                .services
                .run(|_| Ok(yss_project_registry::default_project_parent_directory()?));
            cx.spawn_in(window, async move |view, cx| {
                let result = job.await.ok().and_then(Result::ok);
                let _ = view.update_in(cx, |view, window, cx| {
                    if !std::mem::take(&mut view.default_parent_pending) {
                        return;
                    }
                    match result {
                        Some(path) => {
                            view.original_parent = path.clone();
                            view.parent
                                .update(cx, |input, cx| input.set_value(path, window, cx));
                        }
                        None => {
                            view.error = Some(crate::text::translate(
                                "native.projects.defaultParentFailed",
                            ))
                        }
                    }
                    cx.notify();
                });
            })
            .detach();
        });
        Self {
            services,
            owner,
            lifecycle,
            kind,
            name,
            parent,
            original_name: default_name,
            original_parent: String::new(),
            default_parent_pending: true,
            busy: false,
            progress: None,
            error: None,
            recovery: None,
            confirming: false,
            _subscriptions: subscriptions,
        }
    }
    fn destination(&self, cx: &gpui_kit::App) -> Result<(String, PathBuf), &'static str> {
        let name = self.name.read(cx).value().to_string();
        let name = name.trim();
        let parts = Path::new(name).components().collect::<Vec<_>>();
        if name.is_empty()
            || !matches!(parts.as_slice(), [Component::Normal(part)] if *part == std::ffi::OsStr::new(name))
        {
            return Err(crate::text::t("native.projects.invalidName"));
        }
        let parent = PathBuf::from(self.parent.read(cx).value().as_str());
        if !parent.is_absolute() {
            return Err(crate::text::t("native.projects.parentRequired"));
        }
        Ok((name.to_owned(), parent.join(name)))
    }
    fn choose_parent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(crate::text::t("native.projects.chooseParent").into()),
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = prompt.await;
            let _ = view.update_in(cx, |view, window, cx| {
                view.busy = false;
                match result {
                    Ok(Ok(Some(paths))) => {
                        view.default_parent_pending = false;
                        if let Some(path) = paths.first().and_then(|path| path.to_str()) {
                            view.parent.update(cx, |input, cx| {
                                input.set_value(path.to_owned(), window, cx)
                            });
                        } else {
                            view.error =
                                Some(crate::text::t("native.projects.invalidDirectory").into());
                        }
                    }
                    Ok(Ok(None)) => {}
                    _ => view.error = Some(crate::text::t("native.projects.pickerFailed").into()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn submit(&mut self, recovery: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let command = if recovery {
            let Some(path) = &self.recovery else {
                return;
            };
            ProjectCommand::Open {
                path: path.clone(),
                resource: None,
            }
        } else {
            let (name, destination) = match self.destination(cx) {
                Ok(value) => value,
                Err(error) => {
                    self.error = Some(error.into());
                    cx.notify();
                    return;
                }
            };
            match self.kind {
                ProjectFormKind::Create => ProjectCommand::Create { name, destination },
                ProjectFormKind::SaveAs => ProjectCommand::SaveAs(destination),
            }
        };
        let lifecycle = self.lifecycle;
        let dialog = cx.entity().downgrade();
        let failure = dialog.clone();
        let owner = self.owner.clone();
        let parent = crate::modal_window::owner_window(window, cx);
        self.busy = true;
        self.error = None;
        cx.defer(move |cx| {
            let accepted = parent
                .update(cx, |_, window, cx| {
                    owner
                        .update(cx, |view, cx| {
                            if view.lifecycle != lifecycle || view.is_closing(cx) {
                                return false;
                            }
                            let operation = ProjectOperation::new(command, Some(dialog), view);
                            view.request_project_operation(operation, window, cx);
                            true
                        })
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            if !accepted {
                let _ = failure.update(cx, |form, cx| {
                    form.busy = false;
                    form.error = Some(crate::text::t("native.projects.busy").into());
                    cx.notify();
                });
            }
        });
        cx.notify();
    }

    pub(crate) fn finish(
        &mut self,
        lifecycle: u64,
        error: Option<String>,
        recovery: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        self.lifecycle = lifecycle;
        self.busy = false;
        self.progress = None;
        self.error = error;
        if recovery.is_some() || self.error.is_none() {
            self.recovery = recovery;
        }
        cx.notify();
    }
    pub(crate) fn show_progress(
        &mut self,
        progress: Entity<ProjectProgress>,
        cx: &mut Context<Self>,
    ) {
        self.progress = Some(progress);
        cx.notify();
    }
    pub(crate) fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.busy || self.confirming {
            return false;
        }
        if self.recovery.is_some() {
            return true;
        }
        if self.name.read(cx).value().as_str() == self.original_name
            && self.parent.read(cx).value().as_str() == self.original_parent
        {
            return true;
        }
        self.confirming = true;
        let prompt = crate::modal_window::prompt(
            crate::text::t("native.projects.discardTitle"),
            Some(crate::text::t("native.projects.discardMessage")),
            &[
                crate::text::t("native.projects.discardInput"),
                crate::text::t("native.projects.keepEditing"),
            ],
            window,
            cx,
        );
        cx.spawn_in(window, async move |view, cx| {
            let discard = matches!(prompt.await, Ok(0));
            let _ = view.update_in(cx, |view, window, cx| {
                view.confirming = false;
                if discard {
                    crate::modal_window::close(window, cx);
                }
            });
        })
        .detach();
        false
    }
}
impl Render for ProjectForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(progress) = &self.progress {
            return div()
                .w_full()
                .min_h(gpui_kit::px(230.))
                .flex()
                .items_center()
                .child(progress.clone())
                .into_any_element();
        }
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(if self.kind == ProjectFormKind::Create {
                        crate::text::t("native.projects.createHint")
                    } else {
                        crate::text::t("native.projects.copyHint")
                    }),
            )
            .child(
                div()
                    .text_sm()
                    .child(if self.kind == ProjectFormKind::Create {
                        crate::text::t("native.projects.projectName")
                    } else {
                        crate::text::t("native.projects.copyDirectoryName")
                    }),
            )
            .child(Input::new(&self.name).disabled(self.busy || self.recovery.is_some()))
            .child(
                div()
                    .text_sm()
                    .child(crate::text::t("native.projects.parentDirectory")),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(div().flex_1().child(
                        Input::new(&self.parent).disabled(self.busy || self.recovery.is_some()),
                    ))
                    .child(
                        Button::new("project-parent")
                            .label(crate::text::t("native.projects.choose"))
                            .disabled(self.busy || self.recovery.is_some())
                            .on_click(
                                cx.listener(|view, _, window, cx| view.choose_parent(window, cx)),
                            ),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        self.destination(cx)
                            .map(|(_, path)| path.display().to_string())
                            .unwrap_or_else(|_| {
                                crate::text::t("native.projects.chooseLocation").into()
                            }),
                    ),
            )
            .when_some(self.error.clone(), |view, error| {
                view.child(div().text_sm().text_color(cx.theme().danger).child(error))
            })
            .when_some(self.recovery.as_deref(), |view, path| {
                view.child(super::feedback::recovery_target(path, cx))
            })
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("project-form-cancel")
                            .label(crate::text::t("common.cancel"))
                            .disabled(self.busy)
                            .on_click(cx.listener(|view, _, window, cx| {
                                if view.cancel(window, cx) {
                                    crate::modal_window::close(window, cx);
                                }
                            })),
                    )
                    .child(
                        Button::new("project-form-submit")
                            .primary()
                            .label(if self.busy {
                                crate::text::t("native.assistant.processing")
                            } else if self.recovery.is_some() {
                                crate::text::t("native.projects.openWritten")
                            } else if self.kind == ProjectFormKind::Create {
                                crate::text::t("native.projects.createAndOpen")
                            } else {
                                crate::text::t("native.projects.saveAsAndOpen")
                            })
                            .disabled(self.busy)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.submit(view.recovery.is_some(), window, cx)
                            })),
                    ),
            )
            .into_any_element()
    }
}
