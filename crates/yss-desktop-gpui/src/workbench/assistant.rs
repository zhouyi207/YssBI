//! Project-scoped conversation directory and native panels; DockArea owns placement and visibility.
mod navigation;
mod renaming;

use super::{
    Workbench,
    activity::{ActivityEvent, ActivityPanel, ReadState},
};
use crate::assistant::{ConversationEvent, ConversationPanel, principal};
use gpui::{AppContext, Context, Window, div, prelude::*, px};
use gpui_component::{
    WindowExt,
    dock::{DockPlacement, PaneRef, panel_handle},
};
use std::sync::Arc;
use yss_application::activity_panel::{ActivityItem, ActivityPanelDocument, ActivityRowContent};
use yss_harness_contract::{HarnessSessionId, HarnessSessionRecord};
use yss_project_identity::ProjectResourceKind;

impl Workbench {
    pub(super) fn show_assistant(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.closing {
            return;
        }
        self.ensure_assistant_directory(window, cx);
        self.present_assistant(window, cx);
        if let Some(intent) = self.assistant_intent.take() {
            self.finish_intent(&intent, true, window, cx);
        }
        self.refresh_assistant_directory(window, cx);
    }
    pub(super) fn refresh_assistant_directory(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let panel = self.ensure_assistant_directory(window, cx);
        if self.assistant_reading {
            self.assistant_again = true;
            return;
        }
        self.assistant_generation = self.assistant_generation.wrapping_add(1);
        let generation = self.assistant_generation;
        let lifecycle = self.lifecycle;
        self.assistant_reading = true;
        panel.update(cx, |panel, cx| panel.set_read_state(ReadState::Loading, cx));
        let project = self
            .project
            .as_ref()
            .map(|project| project.identity.clone());
        let services = self.services.clone();
        let job = self.services.executor.spawn(async move {
            services
                .application
                .application
                .assistant_activity_panel(&services.application.harness.host, &principal(), project)
                .await
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.lifecycle != lifecycle || view.assistant_generation != generation {
                    return;
                }
                view.assistant_reading = false;
                if let Some(document) = result {
                    view.resume_conversation(&document, window, cx);
                    view.install_assistant_document(document, window, cx);
                } else {
                    view.activity_read_state(
                        &["assistant"],
                        ReadState::Failed("native.workbench.assistantDirectoryFailed"),
                        cx,
                    );
                    if std::mem::take(&mut view.assistant_reopen) && !view.is_closing(cx) {
                        view.present_assistant(window, cx);
                    }
                    tracing::warn!(
                        code = "native_assistant_directory_failed",
                        "Native conversation directory refresh failed"
                    );
                }
                if view.assistant_again {
                    view.assistant_again = false;
                    view.refresh_assistant_directory(window, cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn invalidate_assistant_directory(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // A new query generation prevents a pre-mutation result from restoring old metadata.
        self.assistant_reading = false;
        self.assistant_again = false;
        self.refresh_assistant_directory(window, cx);
    }

    pub(super) fn install_assistant_document(
        &mut self,
        document: ActivityPanelDocument,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut titles_changed = false;
        for row in &document.rows {
            if let ActivityRowContent::Item(ActivityItem::Conversation {
                session_id, title, ..
            }) = &row.content
                && let Some(panel) = self.conversations.get(session_id)
            {
                titles_changed |= panel.update(cx, |panel, cx| {
                    if let Some(metadata) = &mut panel.session.conversation
                        && metadata.title != *title
                    {
                        metadata.title.clone_from(title);
                        cx.notify();
                        true
                    } else {
                        false
                    }
                });
            }
        }
        if titles_changed {
            self.dock.update(cx, |_, cx| cx.notify());
        }
        let panel = self.ensure_assistant_directory(window, cx);
        panel.update(cx, |panel, cx| {
            panel.replace_document(Arc::new(document), cx)
        });
        self.sync_active_conversation(cx);
    }

    fn ensure_assistant_directory(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Entity<ActivityPanel> {
        if let Some(panel) = self
            .activities
            .get("assistant")
            .and_then(gpui::WeakEntity::upgrade)
        {
            return panel;
        }
        let owner = cx.entity().downgrade();
        let panel = cx.new(|cx| ActivityPanel::pending_conversations(owner, window, cx));
        self.activities.insert("assistant", panel.downgrade());
        self.subscriptions.push(
            cx.subscribe_in(&panel, window, |view, _, event, window, cx| match event {
                ActivityEvent::RefreshResources => view.refresh_assistant_directory(window, cx),
                ActivityEvent::ActivateConversation(id) => {
                    view.activate_conversation(id.clone(), window, cx)
                }
                ActivityEvent::RenameConversation(id, title) => {
                    view.rename_conversation(id.clone(), title.clone(), window, cx)
                }
                ActivityEvent::Tool(id) if id == "newConversation" => {
                    view.new_conversation(window, cx)
                }
                _ => {}
            }),
        );
        self.dock.update(cx, |dock, cx| {
            let selected = dock.layout(DockPlacement::Left).and_then(|tree| {
                let node = tree.find_node(tree.find_panel_node(tree.panels().next()?)?)?;
                match node.kind() {
                    PaneRef::Tabs { panels, active_ix } => panels.get(active_ix).copied(),
                    _ => None,
                }
            });
            // Background directory creation preserves the first group's selection.
            // An explicit show request selects the panel after mounting it.
            dock.add_panel_view(
                panel_handle(panel.clone()),
                DockPlacement::Left,
                None,
                window,
                cx,
            );
            if let Some(selected) = selected {
                dock.select_panel(selected, window, cx);
            }
        });
        panel
    }
    fn present_assistant(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(panel) = self
            .activities
            .get("assistant")
            .and_then(gpui::WeakEntity::upgrade)
        else {
            return;
        };
        self.present_panel(panel_handle(panel), DockPlacement::Left, window, cx);
        if self.project.is_none() {
            self.dock.update(cx, |dock, cx| {
                for placement in [DockPlacement::Right, DockPlacement::Bottom] {
                    if dock.is_dock_open(placement) {
                        dock.toggle_dock(placement, window, cx);
                    }
                }
            });
            let dock = self.dock.clone();
            window.open_dialog(cx, move |dialog, _, _| {
                dialog
                    .title("助手")
                    .width(px(1120.))
                    .overlay_closable(false)
                    .footer(div())
                    .child(div().h(px(620.)).child(dock.clone()))
            });
        }
    }
    pub(super) fn new_conversation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_closing(cx) {
            return;
        }
        let reveal_directory_on_failure = std::mem::take(&mut self.assistant_reopen);
        self.assistant_busy = true;
        let lifecycle = self.lifecycle;
        let services = self.services.clone();
        let job = self.services.executor.spawn(async move {
            services
                .application
                .application
                .create_harness_session(&services.application.harness.host, principal())
                .await
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.lifecycle != lifecycle {
                    return;
                }
                view.assistant_busy = false;
                if let Some(session) = result {
                    view.install_conversation(session, window, cx);
                } else {
                    view.error = Some("新对话未确认，请检查会话目录后重试。".into());
                    if reveal_directory_on_failure {
                        view.present_assistant(window, cx);
                    }
                }
                view.invalidate_assistant_directory(window, cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn open_conversation(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || self.closing || self.assistant_busy {
            return;
        }
        let Ok(session_id) = HarnessSessionId::try_new(id) else {
            return;
        };
        let reveal_directory_on_failure = std::mem::take(&mut self.assistant_reopen);
        self.assistant_busy = true;
        let lifecycle = self.lifecycle;
        let services = self.services.clone();
        let job = self.services.executor.spawn(async move {
            services
                .application
                .application
                .open_harness_session(
                    &services.application.harness.host,
                    &principal(),
                    &session_id,
                )
                .await
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.lifecycle != lifecycle {
                    return;
                }
                view.assistant_busy = false;
                if let Some(session) = result {
                    view.install_conversation(session, window, cx);
                } else {
                    view.error = Some("此对话不可用，请刷新会话目录。".into());
                    if reveal_directory_on_failure {
                        view.present_assistant(window, cx);
                    }
                }
                view.invalidate_assistant_directory(window, cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn install_conversation(
        &mut self,
        session: HarnessSessionRecord,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let id = session.id.to_string();
        self.assistant_closed = None;
        let panel = if let Some(panel) = self.conversations.get(&id) {
            panel.update(cx, |panel, cx| {
                panel.update_session(session);
                cx.notify();
            });
            panel.clone()
        } else {
            let panel =
                cx.new(|cx| ConversationPanel::new(self.services.clone(), session, window, cx));
            self.subscriptions.push(cx.subscribe_in(
                &panel,
                window,
                |view, panel, event, window, cx| {
                    if matches!(
                        event,
                        ConversationEvent::OpenResource(_) | ConversationEvent::OpenResult(_)
                    ) && view.project.as_ref().is_none_or(|project| {
                        &project.identity != panel.read(cx).session.project.project_instance_id()
                    }) {
                        view.error =
                            Some(crate::text::t("panel.assistantResourceUnavailable").into());
                        cx.notify();
                        return;
                    }
                    match event {
                        ConversationEvent::OptionsChanged => {
                            view.dock.update(cx, |_, cx| cx.notify());
                        }
                        ConversationEvent::DirectoryChanged => {
                            view.refresh_assistant_directory(window, cx)
                        }
                        ConversationEvent::Settings => view.show_settings(window, cx),
                        ConversationEvent::OpenResource(resource) => match resource.kind {
                            ProjectResourceKind::Database => {
                                view.open_database(resource.id.clone(), None, window, cx)
                            }
                            ProjectResourceKind::EventGraph
                            | ProjectResourceKind::FunctionGraph => {
                                view.open_graph(resource.id.clone(), window, cx)
                            }
                            ProjectResourceKind::Doc => {
                                view.open_document(resource.id.clone(), None, window, cx)
                            }
                            ProjectResourceKind::Mind => {
                                view.open_mind(resource.id.clone(), None, window, cx)
                            }
                            ProjectResourceKind::Chart => {
                                view.open_chart(resource.id.clone(), None, window, cx)
                            }
                        },
                        ConversationEvent::OpenResult(reference) => {
                            view.open_result(*reference, None, window, cx)
                        }
                    }
                },
            ));
            self.conversations.insert(id.clone(), panel.clone());
            panel
        };
        panel.update(cx, |view, cx| {
            view.set_resource_catalog(
                self.project
                    .as_ref()
                    .map(|project| project.resources.clone()),
                cx,
            )
        });
        self.present_panel(panel_handle(panel), DockPlacement::Center, window, cx);
        self.sync_active_conversation(cx);
    }
    fn visible_conversation(&self, cx: &gpui::App) -> Option<gpui::Entity<ConversationPanel>> {
        let mut active = None;
        let dock = self.dock.read(cx);
        if let Some(tree) = dock.layout(DockPlacement::Center) {
            tree.root().walk(&mut |node| {
                if let PaneRef::Tabs { panels, active_ix } = node.kind()
                    && let Some(panel) = panels.get(active_ix).and_then(|id| dock.panel(*id))
                    && let Ok(panel) = panel.view().downcast::<ConversationPanel>()
                {
                    active = Some(panel);
                }
            });
        }
        active
    }

    pub(super) fn activate_conversation(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || self.closing || self.assistant_busy || self.dock.read(cx).is_locked() {
            return;
        }
        if let Some(panel) = self.visible_conversation(cx)
            && panel.read(cx).session.id.as_str() == id
        {
            self.assistant_reopen = false;
            self.assistant_closed = Some(id);
            self.dock.update(cx, |dock, cx| {
                dock.set_zoomed_out(window, cx);
                dock.remove_panel(panel, window, cx);
            });
            self.sync_active_conversation(cx);
            return;
        }
        self.open_conversation(id, window, cx);
    }

    pub(super) fn sync_active_conversation(&self, cx: &mut Context<Self>) {
        let active = self
            .visible_conversation(cx)
            .map(|panel| panel.read(cx).session.id.to_string());
        if let Some(panel) = self
            .activities
            .get("assistant")
            .and_then(gpui::WeakEntity::upgrade)
        {
            panel.update(cx, |view, cx| {
                view.set_active_resource(active.as_deref(), cx)
            });
        }
    }
}
