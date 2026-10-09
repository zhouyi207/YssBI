//! Authored-file management captures the original version and uses Project's file commands.
use gpui_component::{WindowExt, button::ButtonVariant};
mod panels;
use super::{super::Workbench, AuthoredKind, ResourceAction, names::NameForm};
use crate::file_commands::NativeFile;
use gpui::{ClipboardItem, Context, Entity, Window};
use yss_project::file_resources::{FileCommand, FileSnapshot};
use yss_project_identity::{ProjectInstanceId, ResourceRevision};
use yss_project_model::{
    doc::DocDocument,
    file::{FilePath, FileVersion},
    mind::MindDocument,
};

#[derive(Clone)]
struct FileTarget {
    kind: AuthoredKind,
    project: ProjectInstanceId,
    lifecycle: u64,
    path: String,
    name: String,
    revision: ResourceRevision,
    version: Option<FileVersion>,
}

enum FileResult {
    Document(Option<FileSnapshot<DocDocument>>),
    Mind(Option<FileSnapshot<MindDocument>>),
}

impl Workbench {
    pub(in crate::workbench) fn file_resource_action(
        &mut self,
        kind: AuthoredKind,
        path: String,
        action: ResourceAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_closing(cx) || self.opening.contains(&path) {
            return;
        }
        let Some(project) = &self.project else { return };
        let entry = match kind {
            AuthoredKind::Document => project
                .index
                .docs
                .iter()
                .find(|entry| entry.path.as_str() == path)
                .map(|entry| (entry.name.clone(), entry.revision)),
            AuthoredKind::Mind => project
                .index
                .minds
                .iter()
                .find(|entry| entry.path.as_str() == path)
                .map(|entry| (entry.name.clone(), entry.revision)),
            AuthoredKind::Chart => return,
        };
        let Some((name, revision)) = entry else {
            return;
        };
        let panel = self.file_panel(kind, &path);
        let version = panel.as_ref().map(|panel| panel.version(cx));
        let target = FileTarget {
            kind,
            project: project.identity.clone(),
            lifecycle: self.lifecycle,
            path,
            name,
            revision: version
                .as_ref()
                .map_or(revision, |version| version.revision),
            version,
        };
        if matches!(action, ResourceAction::CopyPath) {
            cx.write_to_clipboard(ClipboardItem::new_string(target.path));
            return;
        }
        if panel.as_ref().is_some_and(|panel| panel.dirty(cx)) {
            self.error = Some(crate::text::translate(
                "native.workbench.saveFileBeforeManaging",
            ));
            cx.notify();
            return;
        }
        match action {
            ResourceAction::Rename => self.resource_name_dialog(
                crate::text::translate("contextMenu.sidebar.rename"),
                target.name.clone(),
                crate::text::translate("contextMenu.dialog.renameSubmit"),
                window,
                cx,
                move |view, name, form, window, cx| {
                    view.mutate_file(target.clone(), action, Some(name), Some(form), window, cx)
                },
            ),
            ResourceAction::Delete => {
                let owner = cx.entity().downgrade();
                window.open_alert_dialog(cx, move |alert, _, _| {
                    let owner = owner.clone();
                    let target = target.clone();
                    alert
                        .title(crate::text::translate("documents.delete"))
                        .description(crate::text::format(
                            "documents.deleteMessage",
                            &[("name", target.name.clone())],
                        ))
                        .confirm()
                        .ok_text(crate::text::translate("common.delete"))
                        .ok_variant(ButtonVariant::Danger)
                        .cancel_text(crate::text::translate("common.cancel"))
                        .on_ok(move |_, window, cx| {
                            let _ = owner.update(cx, |view, cx| {
                                view.mutate_file(target.clone(), action, None, None, window, cx)
                            });
                            true
                        })
                });
            }
            ResourceAction::Duplicate => self.mutate_file(target, action, None, None, window, cx),
            ResourceAction::CopyPath => {}
        }
    }

    fn mutate_file(
        &mut self,
        target: FileTarget,
        action: ResourceAction,
        name: Option<String>,
        form: Option<Entity<NameForm>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_closing(cx)
            || self.lifecycle != target.lifecycle
            || self.opening.contains(&target.path)
            || self
                .project
                .as_ref()
                .is_none_or(|project| project.identity != target.project)
        {
            return;
        }
        let panel = self.file_panel(target.kind, &target.path);
        if let Some(panel) = &panel {
            let version = panel.version(cx);
            if panel.dirty(cx)
                || version.revision != target.revision
                || target
                    .version
                    .as_ref()
                    .is_some_and(|original| *original != version)
            {
                self.error = Some(crate::text::translate("native.workbench.resourceChanged"));
                cx.notify();
                return;
            }
            if !panel.lock(cx) {
                return;
            }
        }
        self.busy = true;
        self.error = None;
        let request = target.clone();
        let publisher = self.services.clone();
        let job = self.services.run(move |services| match request.kind {
            AuthoredKind::Document => {
                mutate::<DocDocument>(services, &publisher, request, action, name)
                    .map(FileResult::Document)
            }
            AuthoredKind::Mind => {
                mutate::<MindDocument>(services, &publisher, request, action, name)
                    .map(FileResult::Mind)
            }
            AuthoredKind::Chart => unreachable!("chart management has its own owner"),
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.lifecycle != target.lifecycle
                    || view
                        .project
                        .as_ref()
                        .is_none_or(|project| project.identity != target.project)
                {
                    NameForm::expired(form.as_ref(), cx);
                    return;
                }
                view.busy = false;
                view.finish_resource_name(
                    form,
                    result
                        .is_none()
                        .then_some("native.workbench.fileResourceFailed"),
                    window,
                    cx,
                );
                match result {
                    Some(result) => {
                        view.accept_file_operation(&target, action, panel, result, window, cx)
                    }
                    None => {
                        if let Some(panel) = panel {
                            panel.unlock(window, cx);
                        }
                    }
                }
                view.refresh_project(window, cx);
                view.persist_layout(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

fn mutate<T: NativeFile>(
    services: &yss_application::runtime::ApplicationServices,
    publisher: &crate::services::NativeServices,
    target: FileTarget,
    action: ResourceAction,
    name: Option<String>,
) -> anyhow::Result<Option<FileSnapshot<T>>> {
    let path = FilePath::<T>::parse(&target.path).map_err(anyhow::Error::msg)?;
    let version = match target.version {
        Some(version) => version,
        None => {
            let snapshot = T::read(services, target.project.clone(), path.clone())?;
            anyhow::ensure!(
                snapshot.version.revision == target.revision,
                "file version changed"
            );
            snapshot.version
        }
    };
    let command = match action {
        ResourceAction::Rename => FileCommand::Rename {
            path,
            version,
            name: name.ok_or_else(|| anyhow::anyhow!("file name missing"))?,
        },
        ResourceAction::Duplicate => FileCommand::Duplicate {
            path,
            version,
            name: None,
        },
        ResourceAction::Delete => FileCommand::Delete { path, version },
        ResourceAction::CopyPath => unreachable!("copy does not mutate a file"),
    };
    let receipt = T::apply_command(services, target.project, command)?;
    publisher.publish_resource(receipt.mutation);
    Ok(receipt.snapshot)
}
