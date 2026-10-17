//! Resource name drafts survive failed submissions and close only after a committed result.
use super::{
    super::{Workbench, name_form::NameForm},
    ResourceAction,
};
use gpui_kit::{Context, Entity, Focusable, SharedString, Window};
use yss_ui::lifecycle::LogReleased;

/// Kind-specific wording for the shared rename and delete prompts.
pub(in crate::workbench) struct ResourcePrompts {
    pub rename_title: SharedString,
    pub delete_title: SharedString,
    pub delete_message: SharedString,
}

impl Workbench {
    /// Rename asks for a name, Delete asks for confirmation and Duplicate commits at once.
    pub(in crate::workbench) fn confirm_resource_action(
        &mut self,
        name: String,
        action: ResourceAction,
        prompts: ResourcePrompts,
        window: &mut Window,
        cx: &mut Context<Self>,
        commit: impl Fn(
            &mut Self,
            Option<String>,
            Option<Entity<NameForm>>,
            &mut Window,
            &mut Context<Self>,
        ) + 'static,
    ) {
        match action {
            ResourceAction::Rename => self.resource_name_dialog(
                prompts.rename_title,
                name,
                crate::text::translate("contextMenu.dialog.renameSubmit"),
                window,
                cx,
                move |view, name, form, window, cx| {
                    commit(view, Some(name), Some(form), window, cx)
                },
            ),
            ResourceAction::Delete => {
                let owner = cx.entity().downgrade();
                crate::modal_window::confirm(
                    prompts.delete_title,
                    prompts.delete_message,
                    crate::text::t("common.delete"),
                    crate::text::t("common.cancel"),
                    window,
                    cx,
                    move |_, window, cx| {
                        owner
                            .update(cx, |view, cx| commit(view, None, None, window, cx))
                            .log_released();
                        true
                    },
                );
            }
            ResourceAction::Duplicate => commit(self, None, None, window, cx),
            ResourceAction::CopyPath => {}
        }
    }

    pub(in crate::workbench) fn resource_name_dialog(
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
            gpui_kit::size(gpui_kit::px(480.), gpui_kit::px(270.)),
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
                        owner
                            .update(cx, |view, cx| {
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
                            })
                            .log_released();
                        false
                    })
                    .cancel(crate::text::translate("common.cancel"))
                    .on_cancel(move |_, _, cx| !cancel.read(cx).busy())
            },
        );
    }

    pub(in crate::workbench) fn finish_resource_name(
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
}
