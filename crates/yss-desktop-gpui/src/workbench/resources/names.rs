//! Resource name drafts survive failed submissions and close only after a committed result.
use super::super::name_form::NameForm;
use super::{super::Workbench, ResourceAction, operations::GraphTarget};
use gpui::{Context, Entity, Focusable, ParentElement, SharedString, Window};
use gpui_component::WindowExt;

impl Workbench {
    pub(super) fn resource_name_dialog(
        &mut self,
        title: impl Into<SharedString>,
        initial: String,
        submit_label: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
        submit: impl Fn(&mut Self, String, Entity<NameForm>, &mut Window, &mut Context<Self>) + 'static,
    ) {
        if self.is_closing(cx) {
            return;
        }
        let Some(project) = self
            .project
            .as_ref()
            .map(|project| project.identity.clone())
        else {
            return;
        };
        let lifecycle = self.lifecycle;
        let owner = cx.entity().downgrade();
        let submit_label = submit_label.into();
        let form = NameForm::new(initial, window, cx);
        let focus = form.focus_handle(cx);
        let title = title.into();
        let submit = std::rc::Rc::new(submit);
        window.open_dialog(cx, move |dialog, _, _| {
            let owner = owner.clone();
            let project = project.clone();
            let form = form.clone();
            let cancel = form.clone();
            let submit = submit.clone();
            dialog
                .title(title.clone())
                .close_button(false)
                .overlay_closable(false)
                .button_props(
                    gpui_component::dialog::DialogButtonProps::default()
                        .ok_text(submit_label.clone())
                        .show_cancel(true)
                        .cancel_text(crate::text::translate("common.cancel")),
                )
                .child(form.clone())
                .on_ok(move |_, window, cx| {
                    let Some(name) = form.read(cx).value(cx) else {
                        return false;
                    };
                    let _ = owner.update(cx, |view, cx| {
                        if view.lifecycle != lifecycle
                            || view.is_closing(cx)
                            || view
                                .project
                                .as_ref()
                                .is_none_or(|current| current.identity != project)
                        {
                            NameForm::fail(Some(&form), "native.workbench.resourceChanged", cx);
                            return;
                        }
                        submit(view, name, form.clone(), window, cx);
                        form.update(cx, |form, cx| {
                            if view.busy {
                                form.submitting(cx);
                            } else {
                                form.finish(Some("native.workbench.resourceChanged"), cx);
                            }
                        });
                    });
                    false
                })
                .on_cancel(move |_, _, cx| !cancel.read(cx).busy())
        });
        window.focus(&focus, cx);
    }

    pub(super) fn finish_resource_name(
        &mut self,
        form: Option<Entity<NameForm>>,
        error: Option<&'static str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(form) = form {
            form.update(cx, |form, cx| {
                form.finish(error, cx);
            });
            if error.is_none() {
                window.close_dialog(cx);
            }
        } else if let Some(error) = error {
            self.error = Some(crate::text::translate(error));
        }
    }

    pub(super) fn rename_graph_dialog(
        &mut self,
        target: GraphTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.resource_name_dialog(
            crate::text::translate("native.workbench.renameGraph"),
            target.name.clone(),
            crate::text::translate("contextMenu.dialog.renameSubmit"),
            window,
            cx,
            move |view, name, form, window, cx| {
                view.mutate_graph_resource(
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
}
