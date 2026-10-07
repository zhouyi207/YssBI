//! Database menus capture the index declaration before the original typed use case runs.
use super::{super::Workbench, ResourceAction};
use crate::project::DesktopProject;
use gpui::{AppContext, ClipboardItem, Context, Window, div, prelude::*};
use gpui_component::{
    WindowExt,
    button::ButtonVariant,
    dialog::DialogButtonProps,
    input::{Input, InputState},
};
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
                self.mutate_database_resource(target, action, None, window, cx)
            }
            ResourceAction::Delete => {
                let owner = cx.entity().downgrade();
                window.open_alert_dialog(cx, move |alert, _, _| {
                    let owner = owner.clone();
                    let target = target.clone();
                    alert
                        .title(format!("删除“{}”？", target.name))
                        .description("该数据库及其数据将被删除，引用它的图节点需要重新选择资源。")
                        .confirm()
                        .ok_text("删除")
                        .ok_variant(ButtonVariant::Danger)
                        .cancel_text("取消")
                        .on_ok(move |_, window, cx| {
                            let _ = owner.update(cx, |view, cx| {
                                view.mutate_database_resource(
                                    target.clone(),
                                    action,
                                    None,
                                    window,
                                    cx,
                                )
                            });
                            true
                        })
                });
            }
        }
    }
    fn rename_database_dialog(
        &mut self,
        target: DatabaseTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.error = None;
        let input = cx.new(|cx| InputState::new(window, cx).default_value(target.name.clone()));
        let owner = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, cx| {
            let busy = owner
                .upgrade()
                .is_none_or(|owner| owner.read(cx).is_closing(cx));
            let error = owner
                .upgrade()
                .and_then(|owner| owner.read(cx).error.clone());
            let mut body = div()
                .flex()
                .flex_col()
                .gap_2()
                .child(Input::new(&input).disabled(busy));
            if let Some(error) = error {
                body = body.child(div().text_xs().child(error));
            }
            let value = input.clone();
            let target = target.clone();
            let confirm = owner.clone();
            let cancel = owner.clone();
            dialog
                .title("重命名数据库")
                .close_button(false)
                .overlay_closable(false)
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("重命名")
                        .show_cancel(true)
                        .cancel_text("取消"),
                )
                .child(body)
                .on_ok(move |_, window, cx| {
                    let name = value.read(cx).value().trim().to_owned();
                    if !name.is_empty() {
                        let _ = confirm.update(cx, |view, cx| {
                            view.mutate_database_resource(
                                target.clone(),
                                ResourceAction::Rename,
                                Some(name),
                                window,
                                cx,
                            )
                        });
                    }
                    false
                })
                .on_cancel(move |_, _, cx| {
                    cancel.upgrade().is_none_or(|owner| !owner.read(cx).busy)
                })
        });
    }
    fn mutate_database_resource(
        &mut self,
        target: DatabaseTarget,
        action: ResourceAction,
        name: Option<String>,
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
            let index = services
                .application
                .query_project_index(request.project.clone(), "zh-CN", true)
                .ok()
                .map(|snapshot| DesktopProject::new(request.project, snapshot));
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
                    return;
                }
                view.busy = false;
                let (success, created, index) = result.unwrap_or((false, None, None));
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
                        ResourceAction::Rename => window.close_dialog(cx),
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
                } else {
                    view.error = Some("资源操作未完成，提交可能已生效，请检查目录后再试。".into());
                }
                view.persist_layout(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
