//! Destination input is a draft; the existing Project use case validates and commits it.
use super::progress::ProjectProgress;
use crate::{
    services::NativeServices,
    workbench::{
        Workbench,
        projects::{ProjectCommand, ProjectOperation},
    },
};
use gpui::{
    AppContext, Context, Entity, IntoElement, PathPromptOptions, PromptLevel, Render, Subscription,
    WeakEntity, Window, div, prelude::*,
};
use gpui_component::{
    ActiveTheme, Disableable, WindowExt,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
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
            "新建项目"
        } else {
            "项目另存为"
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
        let parent =
            cx.new(|cx| InputState::new(window, cx).placeholder("选择父目录，或输入绝对路径"));
        let subscriptions = [&name, &parent]
            .into_iter()
            .map(|input| cx.observe(input, |_, _, cx| cx.notify()))
            .collect();
        cx.defer_in(window, |view, window, cx| {
            let job = view
                .services
                .run(|_| Ok(yss_project_registry::default_project_parent_directory().ok()));
            cx.spawn_in(window, async move |view, cx| {
                if let Some(path) = job.await.ok().and_then(Result::ok).flatten() {
                    let _ = view.update_in(cx, |view, window, cx| {
                        if view.parent.read(cx).value().is_empty() && !view.busy {
                            view.original_parent = path.clone();
                            view.parent
                                .update(cx, |input, cx| input.set_value(path, window, cx));
                        }
                    });
                }
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
            busy: false,
            progress: None,
            error: None,
            recovery: None,
            confirming: false,
            _subscriptions: subscriptions,
        }
    }
    fn destination(&self, cx: &gpui::App) -> Result<(String, PathBuf), &'static str> {
        let name = self.name.read(cx).value().to_string();
        let name = name.trim();
        let parts = Path::new(name).components().collect::<Vec<_>>();
        if name.is_empty()
            || !matches!(parts.as_slice(), [Component::Normal(part)] if *part == std::ffi::OsStr::new(name))
        {
            return Err("请输入单个项目或副本目录名称，不能包含路径分隔符。");
        }
        let parent = PathBuf::from(self.parent.read(cx).value().as_str());
        if !parent.is_absolute() {
            return Err("请选择父目录，或输入绝对路径。");
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
            prompt: Some("选择新项目的父目录".into()),
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = prompt.await;
            let _ = view.update_in(cx, |view, window, cx| {
                view.busy = false;
                match result {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.first().and_then(|path| path.to_str()) {
                            view.parent.update(cx, |input, cx| {
                                input.set_value(path.to_owned(), window, cx)
                            });
                        } else {
                            view.error = Some("目录路径无法识别。".into());
                        }
                    }
                    Ok(Ok(None)) => {}
                    _ => view.error = Some("目录选择器未打开，请重试。".into()),
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
            ProjectCommand::Open(path.clone())
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
        let accepted = self
            .owner
            .update(cx, |view, cx| {
                if view.lifecycle != lifecycle || view.is_closing(cx) {
                    return false;
                }
                let operation = ProjectOperation::new(command, Some(dialog), view);
                view.request_project_operation(operation, window, cx);
                true
            })
            .unwrap_or(false);
        if accepted {
            self.busy = true;
            self.error = None;
        } else {
            self.error = Some("项目正在处理其他操作，输入已保留，请稍后重试。".into());
        }
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
        let prompt = window.prompt(
            PromptLevel::Warning,
            "放弃项目配置？",
            Some("当前填写的名称和目录尚未提交。"),
            &["放弃输入", "继续填写"],
            cx,
        );
        cx.spawn_in(window, async move |view, cx| {
            let discard = matches!(prompt.await, Ok(0));
            let _ = view.update_in(cx, |view, window, cx| {
                view.confirming = false;
                if discard {
                    window.close_dialog(cx);
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
                .min_h(gpui::px(230.))
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
                        "在所选父目录内创建独立项目，成功后打开。"
                    } else {
                        "创建并打开独立副本；当前未保存的更改会先确认保存。"
                    }),
            )
            .child(
                div()
                    .text_sm()
                    .child(if self.kind == ProjectFormKind::Create {
                        "项目名称"
                    } else {
                        "副本目录名称"
                    }),
            )
            .child(Input::new(&self.name).disabled(self.busy || self.recovery.is_some()))
            .child(div().text_sm().child("父目录"))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(div().flex_1().child(
                        Input::new(&self.parent).disabled(self.busy || self.recovery.is_some()),
                    ))
                    .child(
                        Button::new("project-parent")
                            .label("选择…")
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
                            .unwrap_or_else(|_| "请选择项目存放位置".into()),
                    ),
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
                        Button::new("project-form-cancel")
                            .label("取消")
                            .disabled(self.busy)
                            .on_click(cx.listener(|view, _, window, cx| {
                                if view.cancel(window, cx) {
                                    window.close_dialog(cx);
                                }
                            })),
                    )
                    .child(
                        Button::new("project-form-submit")
                            .primary()
                            .label(if self.busy {
                                "正在处理…"
                            } else if self.recovery.is_some() {
                                "打开已写入项目"
                            } else if self.kind == ProjectFormKind::Create {
                                "创建并打开"
                            } else {
                                "另存为并打开"
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
