//! Resource name drafts survive failed submissions and close only after a committed result.
use super::{
    super::{Workbench, name_form::NameForm},
    ResourceAction,
    operations::GraphTarget,
};
use gpui::{Context, Entity, Focusable, SharedString, Window};

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
        crate::modal_window::open(
            title,
            gpui::size(gpui::px(480.), gpui::px(270.)),
            window,
            cx,
            move |window, cx| {
                let form = NameForm::new(initial, window, cx);
                let focus = form.focus_handle(cx);
                let body = form.clone();
                let cancel = form.clone();
                crate::modal_window::ModalContent::new(move |_, _| body.clone())
                    .focus(focus)
                    .confirm(submit_label, move |_, window, cx| {
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
                                form.update(cx, |form, cx| {
                                    form.finish(Some("native.workbench.resourceChanged"), cx);
                                });
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
                    .cancel(crate::text::translate("common.cancel"))
                    .on_cancel(move |_, _, cx| !cancel.read(cx).busy())
            },
        );
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
                crate::modal_window::close_child(window, cx);
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
