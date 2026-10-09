//! Safe native project feedback from the original typed failures and lifecycle receipts.
use yss_application::{
    events::{ProjectLifecycleApplicationEvent, ProjectLifecycleOutcome},
    project::lifecycle::{ApplicationProjectLifecycleError, ProjectLifecycleError},
    session::{SessionCaptureError, SessionRevalidationError},
};

pub(crate) struct ProjectFeedback {
    message: &'static str,
    code: Option<&'static str>,
    destination_written: bool,
}

impl ProjectFeedback {
    pub(crate) fn message(key: &'static str) -> Self {
        Self {
            message: key,
            code: None,
            destination_written: false,
        }
    }

    pub(crate) fn operation(
        error: &anyhow::Error,
        receipt: Option<&ProjectLifecycleApplicationEvent>,
    ) -> Self {
        // Partial-commit guidance takes precedence over the generic command failure.
        if let Some(receipt) = receipt {
            let key = match receipt.outcome {
                ProjectLifecycleOutcome::Committed => None,
                ProjectLifecycleOutcome::RegistryFailed => {
                    Some("native.workbench.projectRegistrationFailed")
                }
                ProjectLifecycleOutcome::ActivationFailed => {
                    Some("native.workbench.projectCopyOpenFailed")
                }
                ProjectLifecycleOutcome::RegistryPending => {
                    Some("native.workbench.projectTrashPartial")
                }
            };
            if let Some(key) = key {
                return Self::message(key);
            }
        }
        let mut feedback =
            if let Some(error) = error.downcast_ref::<ApplicationProjectLifecycleError>() {
                Self::lifecycle(error)
            } else if let Some(error) = error.downcast_ref::<SessionCaptureError>() {
                Self::session(*error)
            } else {
                Self::message("native.workbench.projectOperationFailed")
            };
        feedback.destination_written = receipt.is_some();
        feedback
    }

    fn lifecycle(error: &ApplicationProjectLifecycleError) -> Self {
        use ApplicationProjectLifecycleError as Error;
        match error {
            Error::SessionCapture(error) => Self::session(*error),
            Error::SessionChanged(SessionRevalidationError::Unavailable(error)) => {
                Self::session(*error)
            }
            Error::SessionChanged(SessionRevalidationError::Changed) => {
                Self::message("native.projects.contextChanged")
            }
            Error::SessionRefresh(_) => Self::message("native.projects.sessionRefreshFailed"),
            Error::Lifecycle(error) => match error {
                ProjectLifecycleError::InvalidPath => {
                    Self::message("projectPicker.issues.errors.invalidPath")
                }
                ProjectLifecycleError::ProjectNotFound => {
                    Self::message("projectPicker.issues.errors.projectNotFound")
                }
                ProjectLifecycleError::RegistryLookupFailed(_) => {
                    Self::message("native.projects.loadFailed")
                }
                ProjectLifecycleError::LoadFailed(error)
                | ProjectLifecycleError::AuthorityFailed(error) => {
                    let key = if error.recovery_required() {
                        "projectPicker.issues.errors.recoveryRequired"
                    } else {
                        match error.code() {
                            "invalid_project_root" => "projectPicker.issues.errors.invalidPath",
                            "invalid_graph_document" => {
                                "projectPicker.issues.errors.invalidProject"
                            }
                            "filesystem_transaction_busy"
                            | "project_lifecycle_admission_closed" => {
                                "projectPicker.issues.errors.busy"
                            }
                            "transaction_prepare_failed"
                            | "transaction_commit_failed"
                            | "transaction_rollback_failed" => {
                                "projectPicker.issues.errors.filesystem"
                            }
                            "stale_project_lifecycle"
                            | "stale_resource_lifecycle"
                            | "catalog_resource_stale"
                            | "resource_revision_conflict"
                            | "duplicate_operation" => "native.projects.contextChanged",
                            "invalid_resource_name"
                            | "resource_name_not_normalized"
                            | "resource_name_reserved"
                            | "resource_name_too_long" => "native.projects.invalidProjectName",
                            "resource_name_conflict" => "native.projects.nameConflict",
                            _ => "projectPicker.issues.errors.unknown",
                        }
                    };
                    Self {
                        message: key,
                        code: Some(error.code()),
                        destination_written: false,
                    }
                }
            },
        }
    }

    fn session(error: SessionCaptureError) -> Self {
        Self::message(match error {
            SessionCaptureError::Inactive => "native.projects.contextChanged",
            SessionCaptureError::Replacing => "projectPicker.issues.errors.busy",
            SessionCaptureError::Recovering => "projectPicker.issues.errors.recoveryRequired",
        })
    }

    pub(crate) fn text(&self) -> String {
        let mut message = crate::text::translate(self.message);
        if let Some(code) = self.code {
            message = crate::text::format(
                "native.projects.failureWithCode",
                &[("message", message), ("code", code.into())],
            );
        }
        if self.destination_written {
            message = crate::text::format("native.projects.writtenFailure", &[("error", message)]);
        }
        message
    }
}

pub(crate) fn recovery_target(path: &std::path::Path, cx: &gpui::App) -> impl gpui::IntoElement {
    use gpui::{ClipboardItem, div, prelude::*};
    use gpui_component::{
        ActiveTheme, Sizable,
        button::{Button, ButtonVariants},
        tooltip::Tooltip,
    };
    use gpui_kit_assets::IconName;
    let target = path.display().to_string();
    let tooltip = target.clone();
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_xs()
                .child(crate::text::translate("native.projects.writtenAt")),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .min_w_0()
                .child(
                    div()
                        .id("project-written-target")
                        .flex_1()
                        .min_w_0()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .truncate()
                        .child(target.clone())
                        .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx)),
                )
                .child(
                    Button::new("copy-written-project-target")
                        .small()
                        .ghost()
                        .icon(IconName::Copy)
                        .tooltip(crate::text::translate("native.projects.copyWrittenPath"))
                        .accessibility_label(crate::text::translate(
                            "native.projects.copyWrittenPath",
                        ))
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(target.clone()))
                        }),
                ),
        )
}
