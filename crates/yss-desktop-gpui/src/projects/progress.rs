//! Display the current native project step; Application still owns commit and cancellation.
use gpui_kit::component::{ActiveTheme, Sizable, progress::Progress, tooltip::Tooltip};
use gpui_kit::{
    App, AppContext, Context, Entity, IntoElement, Render, Task, Window, div, prelude::*,
};
use tokio::sync::watch;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ProjectStage {
    Saving,
    Creating,
    Opening,
    Copying,
    Closing,
    Reading,
    Registering,
    Watching,
}

impl ProjectStage {
    fn label(self) -> String {
        crate::text::translate(match self {
            Self::Saving => "native.projects.progress.saving",
            Self::Creating => "projectPicker.loading.creating",
            Self::Opening => "projectPicker.loading.opening",
            Self::Copying => "native.projects.progress.copying",
            Self::Closing => "native.projects.progress.closing",
            Self::Reading => "native.projects.progress.reading",
            Self::Registering => "native.projects.progress.registering",
            Self::Watching => "native.projects.progress.watching",
        })
    }
}

pub(crate) struct ProjectProgress {
    operation: ProjectStage,
    stage: ProjectStage,
    target: Option<String>,
    _delivery: Task<()>,
}

impl ProjectProgress {
    pub(crate) fn start(
        operation: ProjectStage,
        target: Option<String>,
        cx: &mut App,
    ) -> (Entity<Self>, watch::Sender<ProjectStage>) {
        // Only the latest step matters. Slow views never queue work or delay the commit.
        let (sender, mut receiver) = watch::channel(operation);
        let view = cx.new(|cx: &mut Context<Self>| {
            let delivery = cx.spawn(async move |view, cx| {
                while receiver.changed().await.is_ok() {
                    let stage = *receiver.borrow_and_update();
                    if view
                        .update(cx, |view, cx| {
                            if view.stage != stage {
                                view.stage = stage;
                                cx.notify();
                            }
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            });
            Self {
                operation,
                stage: operation,
                target,
                _delivery: delivery,
            }
        });
        (view, sender)
    }
}

impl Render for ProjectProgress {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("project-progress")
            .role(gpui_kit::accesskit::Role::Status)
            .aria_label(self.stage.label())
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_sm().child(self.operation.label()))
            .child(
                Progress::new("project-operation-progress")
                    .small()
                    .loading(true)
                    .accessibility_label(self.stage.label()),
            )
            .when(self.stage != self.operation, |view| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(self.stage.label()),
                )
            })
            .when_some(self.target.clone(), |view, target| {
                view.child(
                    div()
                        .id("project-progress-target")
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .truncate()
                        .child(target.clone())
                        .tooltip(move |window, cx| Tooltip::new(target.clone()).build(window, cx)),
                )
            })
    }
}
