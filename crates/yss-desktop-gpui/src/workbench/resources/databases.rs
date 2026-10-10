//! Database menus capture the index declaration before the original typed use case runs.
use super::super::name_form::NameForm;
use super::{super::Workbench, ResourceAction};
use crate::project::DesktopProject;
use gpui::{ClipboardItem, Context, Entity, Window};
use yss_project_identity::{OperationId, ProjectInstanceId, ResourceRevision};

#[derive(Clone)]
struct DatabaseTarget {
    project: ProjectInstanceId,
    lifecycle: u64,
    id: String,
    revision: ResourceRevision,
    name: String,
    path: String,
}
impl Workbench {
    pub(in crate::workbench) fn database_resource_action(
        &mut self,
        id: String,
        action: ResourceAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_closing(cx) {
            return;
        }
        let Some(project) = &self.project else {
            return;
        };
        let Some(entry) = project.index.databases.iter().find(|entry| entry.id == id) else {
            return;
        };
        let target = DatabaseTarget {
            project: project.identity.clone(),
            lifecycle: self.lifecycle,
            id: id.clone(),
            revision: entry.revision,
            name: entry.name.clone().unwrap_or(id),
            path: entry.resource_path.to_string(),
        };
        match action {
            ResourceAction::CopyPath => {
                cx.write_to_clipboard(ClipboardItem::new_string(target.path))
            }
            ResourceAction::Rename => self.rename_database_dialog(target, window, cx),
            ResourceAction::Duplicate => {
                self.mutate_database_resource(target, action, None, None, window, cx)
            }
            ResourceAction::Delete => {
                let owner = cx.entity().downgrade();
                crate::modal_window::confirm(
                    crate::text::format(
                        "native.workbench.deleteResourceTitle",
                        &[("value0", target.name.to_string())],
                    ),
                    crate::text::t("native.workbench.deleteDatabaseMessage"),
                    crate::text::t("common.delete"),
                    crate::text::t("common.cancel"),
                    window,
                    cx,
                    move |_, window, cx| {
                        let _ = owner.update(cx, |view, cx| {
                            view.mutate_database_resource(
                                target.clone(),
                                action,
                                None,
                                None,
                                window,
                                cx,
                            )
                        });
                        true
                    },
                );
            }
        }
    }
    fn rename_database_dialog(
        &mut self,
        target: DatabaseTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.resource_name_dialog(
            crate::text::translate("native.workbench.renameDatabase"),
            target.name.clone(),
            crate::text::translate("contextMenu.dialog.renameSubmit"),
            window,
            cx,
            move |view, name, form, window, cx| {
                view.mutate_database_resource(
                    target.clone(),
                    ResourceAction::Rename,
                    Some(name),
                    Some(form),
                    window,
                    cx,
                );
            },
        );
    }

    fn mutate_database_resource(
        &mut self,
        target: DatabaseTarget,
        action: ResourceAction,
        name: Option<String>,
        form: Option<Entity<NameForm>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_closing(cx)
            || self.lifecycle != target.lifecycle
            || self
                .project
                .as_ref()
                .is_none_or(|project| project.identity != target.project)
        {
            return;
        }
        self.busy = true;
        self.error = None;
        let publisher = self.services.clone();
        let request = target.clone();
        let job = self.services.run(move |services| {
            let result = match action {
                ResourceAction::Rename => services
                    .application
                    .rename_database_for_application(
                        request.project.clone(),
                        request.id.clone(),
                        request.revision,
                        name.unwrap_or_default(),
                        OperationId::new(),
                    )
                    .map(|receipt| (None, receipt.mutation)),
                ResourceAction::Duplicate => services
                    .application
                    .duplicate_database_for_application(
                        request.project.clone(),
                        request.id.clone(),
                        request.revision,
                        OperationId::new(),
                        None,
                    )
                    .map(|receipt| (Some(receipt.data.id), receipt.mutation)),
                ResourceAction::Delete => services
                    .application
                    .delete_database_for_application(
                        request.project.clone(),
                        request.id,
                        request.revision,
                        OperationId::new(),
                    )
                    .map(|receipt| (None, receipt.mutation)),
                ResourceAction::CopyPath => unreachable!(),
            };
            let mut created = None;
            let success = if let Ok((id, mutation)) = result {
                created = id;
                publisher.publish_resource(mutation);
                true
            } else {
                false
            };
            let language = crate::text::locale();
            let index = services
                .application
                .query_project_index(request.project.clone(), language, true)
                .ok()
                .map(|snapshot| DesktopProject::new(request.project, snapshot, language));
            Ok((success, created, index))
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
                    NameForm::fail(form.as_ref(), "native.workbench.resourceChanged", cx);
                    return;
                }
                view.busy = false;
                let (success, created, index) = result.unwrap_or((false, None, None));
                view.finish_resource_name(
                    form,
                    (!success).then_some("native.workbench.resourceOperationUnconfirmed"),
                    window,
                    cx,
                );
                if let Some(index) = index {
                    view.install_project_index(index, window, cx);
                }
                if matches!(action, ResourceAction::Duplicate | ResourceAction::Delete) {
                    view.rebind_session(window, cx);
                } else {
                    view.refresh_project(window, cx);
                }
                if success {
                    match action {
                        ResourceAction::Rename => {}
                        ResourceAction::Duplicate => {
                            if let Some(id) = created {
                                view.open_database(id, None, window, cx);
                            }
                        }
                        ResourceAction::Delete => {
                            if let Some(editor) = view
                                .databases
                                .remove(&target.id)
                                .and_then(|editor| editor.upgrade())
                            {
                                if view.details.read(cx).database().is_some_and(|current| {
                                    current.entity_id() == editor.entity_id()
                                }) {
                                    view.clear_graph_context(cx);
                                }
                                view.dock
                                    .update(cx, |dock, cx| dock.remove_panel(editor, window, cx));
                            }
                        }
                        ResourceAction::CopyPath => {}
                    }
                }
                view.persist_layout(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
