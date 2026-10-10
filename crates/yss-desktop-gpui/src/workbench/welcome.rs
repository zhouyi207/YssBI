//! Empty-workspace quick actions and recent opens; no project management or separate layout.
use super::{
    Workbench,
    chrome::{OpenProjectDirectory, OpenRecentProject},
};
use crate::projects::form::ProjectFormKind;
use gpui::{App, Context, IntoElement, SharedString, Window, div, prelude::*, px, relative};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable,
    button::{Button, ButtonVariants},
    kbd::Kbd,
};
use gpui_kit_assets::IconName;

impl Workbench {
    pub(super) fn render_welcome(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        div()
            .size_full()
            .overflow_hidden()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .id("welcome-content")
                    .w_full()
                    .max_w(px(440.))
                    .max_h_full()
                    .overflow_y_scroll()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .child(
                                Icon::new(IconName::Workflow)
                                    .size(px(24.))
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .child("YssBI"),
                            ),
                    )
                    .child(self.render_welcome_actions(window, cx))
                    .child(self.render_welcome_recent(cx)),
            )
    }

    fn render_welcome_actions(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let disabled = self.is_closing(cx);
        div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .child(section_header(
                crate::text::t("native.workbench.getStarted"),
                cx,
            ))
            .child(
                quick_action(
                    "welcome-open",
                    crate::text::t("native.workbench.openProjectDirectory"),
                    IconName::FolderOpen,
                    Kbd::binding_for_action(&OpenProjectDirectory, Some("Workbench"), window),
                    cx,
                )
                .disabled(disabled)
                .on_click(cx.listener(|view, _, window, cx| view.choose_project(window, cx))),
            )
            .child(
                quick_action(
                    "welcome-new",
                    crate::text::t("native.workbench.newProject"),
                    IconName::Plus,
                    None,
                    cx,
                )
                .disabled(disabled)
                .on_click(cx.listener(|view, _, window, cx| {
                    view.project_form(ProjectFormKind::Create, window, cx)
                })),
            )
            .child(
                quick_action(
                    "welcome-recent",
                    crate::text::t("native.workbench.openRecentProjects"),
                    IconName::Search,
                    Kbd::binding_for_action(&OpenRecentProject, Some("Workbench"), window),
                    cx,
                )
                .disabled(disabled)
                .on_click(cx.listener(|view, _, window, cx| view.open_recent_projects(window, cx))),
            )
    }

    fn render_welcome_recent(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let snapshot = self.recent.read(cx).snapshot();
        let disabled = self.is_closing(cx);
        let mut recent = div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .child(section_header(
                crate::text::t("native.workbench.recentProjects"),
                cx,
            ));
        if snapshot.loading {
            recent = recent.child(
                div()
                    .px_2()
                    .py_3()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(crate::text::t("native.workbench.loadingRecentProjects")),
            );
        } else if let Some(error) = snapshot.error {
            recent = recent
                .child(
                    div()
                        .px_2()
                        .py_3()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(crate::text::translate(error)),
                )
                .child(
                    Button::new("welcome-retry")
                        .small()
                        .ghost()
                        .label(crate::text::t("common.retry"))
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.recent.update(cx, |recent, cx| recent.reload(cx))
                        })),
                );
        } else if snapshot.records.is_empty() {
            recent = recent.child(
                div()
                    .px_2()
                    .py_3()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(crate::text::t("native.workbench.recentProjectsEmpty")),
            );
        } else {
            for record in snapshot.records.iter().take(6) {
                let record = record.clone();
                let generation = snapshot.generation;
                recent = recent.child(
                    Button::new(SharedString::from(format!(
                        "welcome-{}",
                        record.id.as_str()
                    )))
                    .small()
                    .ghost()
                    .w_full()
                    .h(px(30.))
                    .px_2()
                    .disabled(disabled)
                    .accessibility_label(crate::text::format(
                        "native.workbench.openRecentProjectHint",
                        &[
                            ("value0", record.name.to_string()),
                            ("value1", record.path.to_string()),
                        ],
                    ))
                    .tooltip(record.path.clone())
                    .child(
                        div()
                            .w_full()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Icon::new(IconName::Folder)
                                    .size_4()
                                    .flex_shrink_0()
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .text_sm()
                                            .truncate()
                                            .child(record.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .w(relative(0.58))
                                            .flex_shrink_0()
                                            .text_xs()
                                            .truncate()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(record.path.clone()),
                                    ),
                            ),
                    )
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.open_recent_record(record.clone(), generation, window, cx)
                    })),
                );
            }
        }
        recent
    }
}

fn section_header(title: &'static str, cx: &App) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .px_2()
        .mb_1()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(title)
        .child(div().flex_1().h(px(1.)).bg(cx.theme().border))
}

fn quick_action(
    id: &'static str,
    label: &'static str,
    icon: IconName,
    shortcut: Option<Kbd>,
    cx: &App,
) -> Button {
    Button::new(id)
        .small()
        .ghost()
        .w_full()
        .h(px(30.))
        .px_2()
        .accessibility_label(label)
        .child(
            div()
                .w_full()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Icon::new(icon)
                        .size_4()
                        .text_color(cx.theme().muted_foreground),
                )
                .child(div().flex_1().text_left().text_sm().child(label))
                .when_some(shortcut, |view, shortcut| view.child(shortcut)),
        )
}
