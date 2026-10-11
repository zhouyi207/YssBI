use super::{Failure, commands::Mutation};
use crate::{settings::SettingsPanel, text::translate as t};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    menu::{DropdownMenu, PopupMenuItem},
    setting::{SettingGroup, SettingItem},
};
use gpui_kit::{AnyElement, Context, IntoElement, SharedString, div, prelude::*};
use yss_harness_contract::{ProjectKnowledgeSourceSummary, ProjectKnowledgeStatus};
use yss_project_identity::ProjectInstanceId;
use yss_project_model::doc::DocPath;

impl SettingsPanel {
    pub(in crate::settings) fn knowledge_status(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut content = div().flex_shrink_0().flex().flex_col().gap_2();
        if self.knowledge.scope.is_some() {
            if let Some(failure) = self.knowledge.failure {
                content = content.child(
                    div()
                        .py_2()
                        .flex()
                        .items_center()
                        .gap_3()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(div().flex_1().child(t(match failure {
                            Failure::Load => "settings.knowledge.loadFailed",
                            Failure::Mutation => "settings.knowledge.failed",
                        })))
                        .when(failure == Failure::Load, |view| {
                            view.child(
                                Button::new("knowledge-retry")
                                    .small()
                                    .ghost()
                                    .label(t("common.retry"))
                                    .disabled(self.knowledge_busy())
                                    .on_click(
                                        cx.listener(|view, _, _, cx| view.refresh_knowledge(cx)),
                                    ),
                            )
                        }),
                );
            }
            if self.knowledge.pending || self.knowledge.loading {
                content =
                    content.child(div().py_2().text_sm().child(t(if self.knowledge.pending {
                        "settings.knowledge.updating"
                    } else {
                        "settings.knowledge.loading"
                    })));
            }
            if !self.knowledge.loading
                && !self.knowledge.pending
                && self.knowledge.sources.is_empty()
                && self.knowledge.failure != Some(Failure::Load)
            {
                content = content.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(t("settings.knowledge.empty")),
                );
            }
        } else {
            content = content
                .child(div().text_sm().child(t("settings.knowledge.noProject")))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(t("settings.knowledge.builtin")),
                );
        }
        content.into_any_element()
    }

    pub(in crate::settings) fn knowledge_page(&self, cx: &mut Context<Self>) -> Vec<SettingGroup> {
        let Some(scope) = &self.knowledge.scope else {
            return Vec::new();
        };
        let project = scope.identity.clone();
        let mut groups = vec![
            SettingGroup::new()
                .item(self.render_field(
                    t("settings.knowledge.document"),
                    t("settings.knowledge.description"),
                    move |view, cx| view.knowledge_documents(&project, cx),
                    cx,
                ))
                .footer(|_, _| t("settings.knowledge.builtin")),
        ];
        if !self.knowledge.sources.is_empty() {
            groups.push(
                SettingGroup::new().items(
                    self.knowledge
                        .sources
                        .iter()
                        .map(|source| self.knowledge_source(&scope.identity, source, cx)),
                ),
            );
        }
        groups
    }

    fn knowledge_documents(
        &self,
        project: &ProjectInstanceId,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let documents = self.knowledge.available_documents();
        let selected = documents
            .iter()
            .find(|doc| self.knowledge.selected.as_ref() == Some(&doc.path));
        let label = selected
            .map(|doc| format!("{} · {}", doc.name, doc.path.as_str()))
            .unwrap_or_else(|| {
                t(if documents.is_empty() {
                    "settings.knowledge.noDocuments"
                } else {
                    "settings.knowledge.choose"
                })
            });
        let choices: Vec<_> = documents
            .into_iter()
            .map(|doc| {
                (
                    doc.path.clone(),
                    format!("{} · {}", doc.name, doc.path.as_str()),
                )
            })
            .collect();
        let generation = self.knowledge.generation;
        let owner = cx.weak_entity();
        let expected = project.clone();
        let selector = Button::new("knowledge-document")
            .w_full()
            .label(label)
            .disabled(self.knowledge_busy() || choices.is_empty())
            .dropdown_menu(move |mut menu, _, _| {
                for (path, label) in &choices {
                    let path = path.clone();
                    let expected = expected.clone();
                    let owner = owner.clone();
                    menu =
                        menu.item(PopupMenuItem::new(label.clone()).on_click(move |_, _, cx| {
                            let _ = owner.update(cx, |view, cx| {
                                view.select_knowledge_document(
                                    &expected,
                                    generation,
                                    path.clone(),
                                    cx,
                                );
                            });
                        }));
                }
                menu
            });
        let expected = project.clone();
        div()
            .flex()
            .flex_col()
            .items_start()
            .gap_2()
            .child(selector)
            .child(
                Button::new("knowledge-add")
                    .small()
                    .primary()
                    .icon(IconName::Plus)
                    .label(t("settings.knowledge.add"))
                    .disabled(self.knowledge_busy() || self.knowledge.selected.is_none())
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if let Some(path) = view.knowledge.selected.clone() {
                            view.mutate_knowledge(
                                &expected,
                                generation,
                                Mutation::Rebuild(path),
                                cx,
                            );
                        }
                    })),
            )
            .into_any_element()
    }

    fn knowledge_source(
        &self,
        project: &ProjectInstanceId,
        source: &ProjectKnowledgeSourceSummary,
        cx: &mut Context<Self>,
    ) -> SettingItem {
        let status = match source.status {
            ProjectKnowledgeStatus::Ready => "settings.knowledge.status.ready",
            ProjectKnowledgeStatus::Changed => "settings.knowledge.status.changed",
            ProjectKnowledgeStatus::Empty => "settings.knowledge.status.empty",
            ProjectKnowledgeStatus::Unavailable => "settings.knowledge.status.unavailable",
        };
        let updated = i64::try_from(source.updated_at.get())
            .ok()
            .and_then(chrono::DateTime::from_timestamp_millis)
            .map(|time| time.naive_utc().format("%Y-%m-%d %H:%M").to_string());
        let description = match updated {
            Some(updated) => format!(
                "{}\n{}\n{}",
                source.path,
                t(status),
                crate::text::format("settings.knowledge.updated", &[("value", updated)]),
            ),
            None => format!("{}\n{}", source.path, t(status)),
        };
        let project = project.clone();
        let source_id = source.source_id.clone();
        let open_button: SharedString = format!("knowledge-open-{}", source_id.as_str()).into();
        let rebuild_button: SharedString =
            format!("knowledge-rebuild-{}", source_id.as_str()).into();
        let remove_button: SharedString = format!("knowledge-remove-{}", source_id.as_str()).into();
        self.render_field(
            source.title.clone(),
            description,
            move |view, cx| {
                let Some(source) = view
                    .knowledge
                    .sources
                    .iter()
                    .find(|source| source.source_id == source_id)
                else {
                    return div().into_any_element();
                };
                let generation = view.knowledge.generation;
                let busy = view.knowledge_busy();
                let unavailable = source.status == ProjectKnowledgeStatus::Unavailable;
                let open_project = project.clone();
                let open_id = source_id.clone();
                let rebuild_project = project.clone();
                let path = source.path.clone();
                let remove_project = project.clone();
                let remove_id = source_id.clone();
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        Button::new(open_button.clone())
                            .small()
                            .label(t("settings.knowledge.open"))
                            .disabled(busy || unavailable)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.open_knowledge_document(
                                    &open_project,
                                    generation,
                                    &open_id,
                                    cx,
                                )
                            })),
                    )
                    .child(
                        Button::new(rebuild_button.clone())
                            .small()
                            .label(t("settings.knowledge.rebuild"))
                            .disabled(busy || unavailable)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                if let Ok(path) = DocPath::parse(&path) {
                                    view.mutate_knowledge(
                                        &rebuild_project,
                                        generation,
                                        Mutation::Rebuild(path),
                                        cx,
                                    );
                                }
                            })),
                    )
                    .child(
                        Button::new(remove_button.clone())
                            .small()
                            .ghost()
                            .label(t("settings.knowledge.remove"))
                            .disabled(busy)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.mutate_knowledge(
                                    &remove_project,
                                    generation,
                                    Mutation::Remove(remove_id.clone()),
                                    cx,
                                )
                            })),
                    )
                    .into_any_element()
            },
            cx,
        )
    }
}
