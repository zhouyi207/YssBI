//! Native project transitions use the existing Application lifecycle and one installation path.
mod operations;

use super::{Workbench, lifecycle::AfterSave};
use crate::projects::{
    RecentDelegate, RecentProjectEvent,
    form::{ProjectForm, ProjectFormKind},
    progress::{ProjectProgress, ProjectStage},
};
use gpui::{AppContext, Context, Focusable, PathPromptOptions, WeakEntity, Window, prelude::*, px};
use gpui_component::{
    IndexPath,
    list::{List, ListDelegate, ListState},
};
use std::path::PathBuf;
use yss_project_identity::ProjectInstanceId;
use yss_project_registry_contract::ProjectRecord;

pub(crate) enum ProjectCommand {
    Open {
        path: PathBuf,
        resource: Option<String>,
    },
    Create {
        name: String,
        destination: PathBuf,
    },
    SaveAs(PathBuf),
    Close,
    Exit,
}
pub(crate) struct ProjectOperation {
    command: ProjectCommand,
    project: Option<ProjectInstanceId>,
    dialog: Option<WeakEntity<ProjectForm>>,
}
impl ProjectOperation {
    pub(super) fn show_progress(
        &self,
        progress: gpui::Entity<ProjectProgress>,
        cx: &mut Context<Workbench>,
    ) {
        if let Some(dialog) = &self.dialog {
            let _ = dialog.update(cx, |form, cx| form.show_progress(progress, cx));
        }
    }
    pub(crate) fn new(
        command: ProjectCommand,
        dialog: Option<WeakEntity<ProjectForm>>,
        view: &Workbench,
    ) -> Self {
        Self {
            command,
            project: view
                .project
                .as_ref()
                .map(|project| project.identity.clone()),
            dialog,
        }
    }
    pub(super) fn fail(&self, lifecycle: u64, message: &str, cx: &mut Context<Workbench>) {
        if let Some(dialog) = &self.dialog {
            let _ = dialog.update(cx, |view, cx| {
                view.finish(lifecycle, Some(message.into()), None, cx)
            });
        }
    }
}
impl ProjectCommand {
    fn progress(
        &self,
        cx: &mut gpui::App,
    ) -> (
        gpui::Entity<ProjectProgress>,
        tokio::sync::watch::Sender<ProjectStage>,
    ) {
        let (stage, target) = match self {
            Self::Open { path, .. } => (ProjectStage::Opening, Some(path)),
            Self::Create { destination, .. } => (ProjectStage::Creating, Some(destination)),
            Self::SaveAs(path) => (ProjectStage::Copying, Some(path)),
            Self::Close => (ProjectStage::Closing, None),
            Self::Exit => unreachable!("window exit has no project commit"),
        };
        ProjectProgress::start(stage, target.map(|path| path.display().to_string()), cx)
    }
}
impl Workbench {
    pub(super) fn connect_recent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.recent_subscription = Some(cx.subscribe_in(
            &self.recent,
            window,
            |view, _, event, window, cx| {
                match event {
                    RecentProjectEvent::Changed => {
                        let snapshot = view.recent.read(cx).snapshot();
                        if let Some(picker) = view
                            .recent_picker
                            .as_ref()
                            .and_then(gpui::WeakEntity::upgrade)
                        {
                            crate::modal_window::update_child(window, cx, move |window, cx| {
                                picker.update(cx, |picker, cx| {
                                    let selected = picker.delegate_mut().install(snapshot);
                                    picker.set_selected_index(selected, window, cx);
                                    cx.notify();
                                });
                            });
                        }
                        cx.notify();
                    }
                    RecentProjectEvent::Open {
                        record,
                        generation,
                        picker,
                    } => {
                        if view
                            .recent_picker
                            .as_ref()
                            .is_some_and(|current| current.entity_id() == *picker)
                            && !view.is_closing(cx)
                            && view.recent.read(cx).can_open(record, *generation)
                        {
                            view.recent_picker = None;
                            crate::modal_window::close_child(window, cx);
                            view.open_recent_record(record.clone(), *generation, window, cx);
                        }
                    }
                    RecentProjectEvent::Dismiss(picker) => {
                        if view
                            .recent_picker
                            .as_ref()
                            .is_some_and(|current| current.entity_id() == *picker)
                        {
                            view.recent_picker = None;
                            crate::modal_window::close_child(window, cx);
                        }
                    }
                }
            },
        ));
    }

    pub(super) fn open_recent_projects(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_closing(cx) {
            return;
        }
        if let Some(picker) = self
            .recent_picker
            .as_ref()
            .and_then(gpui::WeakEntity::upgrade)
        {
            crate::modal_window::update_child(window, cx, move |window, cx| {
                window.activate_window();
                picker.update(cx, |picker, cx| picker.focus(window, cx));
            });
            return;
        }
        self.recent.update(cx, |recent, cx| recent.reload(cx));
        let recent = self.recent.clone();
        let owner = cx.entity().downgrade();
        crate::modal_window::open(
            crate::text::t("native.workbench.openRecentProject"),
            gpui::size(px(680.), px(510.)),
            window,
            cx,
            move |window, cx| {
                let delegate = RecentDelegate::new(recent.clone(), recent.read(cx).snapshot());
                let picker = cx.new(|cx| ListState::new(delegate, window, cx).searchable(true));
                picker.update(cx, |picker, cx| {
                    if picker.delegate().items_count(0, cx) > 0 {
                        picker.set_selected_index(Some(IndexPath::default()), window, cx);
                    }
                });
                let picker_id = picker.entity_id();
                let _ = owner.update(cx, |view, _| view.recent_picker = Some(picker.downgrade()));
                let focus = picker.read(cx).focus_handle(cx);
                crate::modal_window::ModalContent::new(move |_, _| {
                    gpui::div().h(px(400.)).child(
                        List::new(&picker)
                            .search_placeholder(crate::text::t("native.workbench.searchProjects")),
                    )
                })
                .without_buttons()
                .focus(focus)
                .on_closed(move |cx| {
                    cx.defer(move |cx| {
                        let _ = owner.update(cx, |view, _| {
                            if view
                                .recent_picker
                                .as_ref()
                                .is_some_and(|current| current.entity_id() == picker_id)
                            {
                                view.recent_picker = None;
                            }
                        });
                    });
                })
            },
        );
    }

    pub(super) fn open_recent_record(
        &mut self,
        record: ProjectRecord,
        generation: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_closing(cx) || !self.recent.read(cx).can_open(&record, generation) {
            return;
        }
        let operation = ProjectOperation::new(
            ProjectCommand::Open {
                path: record.path.into(),
                resource: None,
            },
            None,
            self,
        );
        self.request_project_operation(operation, window, cx);
    }
    pub(super) fn project_form(
        &mut self,
        kind: ProjectFormKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_closing(cx) || (kind == ProjectFormKind::SaveAs && self.project.is_none()) {
            return;
        }
        let name = match kind {
            ProjectFormKind::Create => crate::text::t("projectPicker.newProjectModal.title").into(),
            ProjectFormKind::SaveAs => crate::text::format(
                "native.workbench.copyName",
                &[(
                    "value0",
                    self.project
                        .as_ref()
                        .unwrap()
                        .index
                        .project_name
                        .to_string(),
                )],
            ),
        };
        let owner = cx.entity().downgrade();
        let services = self.services.clone();
        let lifecycle = self.lifecycle;
        crate::modal_window::open(
            kind.title(),
            gpui::size(px(660.), px(420.)),
            window,
            cx,
            move |window, cx| {
                let editor = cx
                    .new(|cx| ProjectForm::new(services, owner, lifecycle, kind, name, window, cx));
                let cancel = editor.clone();
                crate::modal_window::ModalContent::new(move |_, _| editor.clone())
                    .without_buttons()
                    .on_cancel(move |_, window, cx| {
                        cancel.update(cx, |view, cx| view.cancel(window, cx))
                    })
            },
        );
    }

    pub(super) fn choose_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_closing(cx) {
            return;
        }
        let lifecycle = self.lifecycle;
        self.closing = true;
        cx.notify();
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(crate::text::t("native.workbench.chooseProjectDirectory").into()),
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = prompt.await;
            let _ = view.update_in(cx, |view, window, cx| {
                if view.lifecycle != lifecycle {
                    return;
                }
                view.closing = false;
                match result {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.into_iter().next() {
                            let operation = ProjectOperation::new(
                                ProjectCommand::Open {
                                    path,
                                    resource: None,
                                },
                                None,
                                view,
                            );
                            view.request_project_operation(operation, window, cx);
                        }
                    }
                    Ok(Ok(None)) => {}
                    _ => {
                        view.error =
                            Some(crate::text::t("native.workbench.projectPickerFailed").into())
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn close_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let operation = ProjectOperation::new(ProjectCommand::Close, None, self);
        self.request_project_operation(operation, window, cx);
    }
    pub(crate) fn request_project_operation(
        &mut self,
        operation: ProjectOperation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_closing(cx) {
            operation.fail(
                self.lifecycle,
                crate::text::t("native.workbench.waitForTask"),
                cx,
            );
            return;
        }
        let changes = self.has_unsaved(cx);
        if !changes {
            self.perform_project_operation(operation, window, cx);
            return;
        }
        self.closing = true;
        let lifecycle = self.lifecycle;
        let save_as = matches!(operation.command, ProjectCommand::SaveAs(_));
        let message = if save_as {
            crate::text::t("native.workbench.saveBeforeCopy")
        } else {
            crate::text::t("native.workbench.continueConfirmation")
        };
        let buttons: &[&str] = if save_as {
            &[
                crate::text::t("native.workbench.saveAndContinue"),
                crate::text::t("common.cancel"),
            ]
        } else {
            &[
                crate::text::t("native.workbench.saveAndContinue"),
                crate::text::t("native.workbench.continueWithoutSaving"),
                crate::text::t("common.cancel"),
            ]
        };
        let prompt = crate::modal_window::prompt(
            crate::text::t("native.workbench.unsavedChangesTitle"),
            Some(message),
            buttons,
            window,
            cx,
        );
        cx.spawn_in(window, async move |view, cx| {
            let choice = prompt.await.ok();
            let _ = view.update_in(cx, |view, window, cx| {
                if view.lifecycle != lifecycle {
                    return;
                }
                view.closing = false;
                match choice {
                    Some(0) => view.save_all(AfterSave::Project(Box::new(operation)), window, cx),
                    Some(1) if !save_as => {
                        view.settings
                            .update(cx, |settings, cx| settings.discard(cx));
                        view.perform_project_operation(operation, window, cx);
                    }
                    _ => operation.fail(
                        view.lifecycle,
                        crate::text::t("native.workbench.cancelledInputPreserved"),
                        cx,
                    ),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
