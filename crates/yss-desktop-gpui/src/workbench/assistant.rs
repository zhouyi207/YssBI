//! Project-scoped conversation directory and native panels; DockArea owns placement and visibility.
use super::{
    Workbench,
    activity::{ActivityEvent, ActivityPanel},
};
use crate::assistant::{ConversationEvent, ConversationPanel, principal};
use gpui::{AppContext, Context, Window, div, prelude::*, px};
use gpui_component::{
    WindowExt,
    dock::{DockPlacement, panel_handle},
    input::{Input, InputState},
};
use std::sync::Arc;
use yss_application::activity_panel::ActivityPanelDocument;
use yss_harness_contract::{HarnessSessionId, HarnessSessionRecord};
use yss_project_identity::ProjectResourceKind;

impl Workbench {
    pub(super) fn show_assistant(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.closing {
            return;
        }
        self.refresh_assistant_directory(true, window, cx);
    }
    pub(super) fn refresh_assistant_directory(
        &mut self,
        reveal: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.assistant_reveal |= reveal;
        if self.assistant_reading {
            self.assistant_again = true;
            return;
        }
        self.assistant_generation = self.assistant_generation.wrapping_add(1);
        let generation = self.assistant_generation;
        let lifecycle = self.lifecycle;
        self.assistant_reading = true;
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
                    view.install_assistant_document(document, window, cx);
                    if view.assistant_reveal {
                        view.assistant_reveal = false;
                        view.present_assistant(window, cx);
                        if let Some(intent) = view.assistant_intent.take() {
                            view.finish_intent(&intent, true, window, cx);
                        }
                    }
                } else {
                    view.assistant_reveal = false;
                    view.error = Some("助手目录读取失败，请重新打开助手。".into());
                    if let Some(intent) = view.assistant_intent.take() {
                        view.finish_intent(&intent, false, window, cx);
                    }
                }
                if view.assistant_again {
                    view.assistant_again = false;
                    view.refresh_assistant_directory(false, window, cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn install_assistant_document(
        &mut self,
        document: ActivityPanelDocument,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(panel) = self
            .activities
            .get("assistant")
            .and_then(gpui::WeakEntity::upgrade)
        {
            panel.update(cx, |view, cx| view.replace_document(Arc::new(document), cx));
            return;
        }
        let panel = cx.new(|cx| ActivityPanel::with_search(Arc::new(document), window, cx));
        self.activities.insert("assistant", panel.downgrade());
        self.subscriptions.push(
            cx.subscribe_in(&panel, window, |view, _, event, window, cx| match event {
                ActivityEvent::OpenConversation(id) => {
                    view.open_conversation(id.clone(), window, cx)
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
            dock.add_panel_view(panel_handle(panel), DockPlacement::Left, None, window, cx)
        });
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
                }
                view.refresh_assistant_directory(false, window, cx);
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
        if let Some(panel) = self.conversations.get(&id) {
            self.present_panel(
                panel_handle(panel.clone()),
                DockPlacement::Center,
                window,
                cx,
            );
            self.mark_conversation(&id, cx);
            return;
        }
        let Ok(session_id) = HarnessSessionId::try_new(id) else {
            return;
        };
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
                }
                view.refresh_assistant_directory(false, window, cx);
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
        let panel = if let Some(panel) = self.conversations.get(&id) {
            panel.clone()
        } else {
            let panel =
                cx.new(|cx| ConversationPanel::new(self.services.clone(), session, window, cx));
            self.subscriptions.push(cx.subscribe_in(
                &panel,
                window,
                |view, _, event, window, cx| match event {
                    ConversationEvent::DirectoryChanged => {
                        view.refresh_assistant_directory(false, window, cx)
                    }
                    ConversationEvent::Settings => view.show_settings(window, cx),
                    ConversationEvent::OpenResource(resource) => match resource.kind {
                        ProjectResourceKind::Database => {
                            view.open_database(resource.id.clone(), None, window, cx)
                        }
                        ProjectResourceKind::EventGraph | ProjectResourceKind::FunctionGraph => {
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
                },
            ));
            self.conversations.insert(id.clone(), panel.clone());
            panel
        };
        self.present_panel(panel_handle(panel), DockPlacement::Center, window, cx);
        self.mark_conversation(&id, cx);
    }
    fn mark_conversation(&self, id: &str, cx: &mut Context<Self>) {
        if let Some(panel) = self
            .activities
            .get("assistant")
            .and_then(gpui::WeakEntity::upgrade)
        {
            panel.update(cx, |view, cx| view.set_active_resource(Some(id), cx));
        }
    }
    pub(super) fn rename_conversation(
        &mut self,
        id: String,
        title: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_closing(cx) {
            return;
        }
        let Ok(session) = HarnessSessionId::try_new(id.clone()) else {
            return;
        };
        let input = cx.new(|cx| InputState::new(window, cx).default_value(title));
        self.error = None;
        let owner = cx.entity().downgrade();
        let lifecycle = self.lifecycle;
        window.open_dialog(cx, move |dialog, _, cx| {
            let busy = owner
                .upgrade()
                .is_some_and(|view| view.read(cx).assistant_busy);
            let error = owner.upgrade().and_then(|view| view.read(cx).error.clone());
            let cancel = owner.clone();
            let input = input.clone();
            let owner = owner.clone();
            let session = session.clone();
            let id = id.clone();
            dialog
                .title("重命名会话")
                .width(px(440.))
                .overlay_closable(false)
                .child(Input::new(&input).disabled(busy))
                .when_some(error, |dialog, error| {
                    dialog.child(div().text_sm().child(error))
                })
                .on_cancel(move |_, _, cx| {
                    !cancel
                        .upgrade()
                        .is_some_and(|view| view.read(cx).assistant_busy)
                })
                .on_ok(move |_, window, cx| {
                    let title = input.read(cx).value().trim().to_owned();
                    if title.is_empty() {
                        return false;
                    }
                    let session = session.clone();
                    let id = id.clone();
                    let _ = owner.update(cx, |view, cx| {
                        if view.lifecycle != lifecycle || view.is_closing(cx) {
                            return;
                        }
                        view.assistant_busy = true;
                        let services = view.services.clone();
                        let job = view.services.executor.spawn(async move {
                            services
                                .application
                                .application
                                .rename_harness_session(
                                    &services.application.harness.host,
                                    &principal(),
                                    &session,
                                    title,
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
                                    window.close_dialog(cx);
                                    if let Some(panel) = view.conversations.get(&id) {
                                        panel.update(cx, |view, cx| {
                                            view.session = session;
                                            cx.notify();
                                        });
                                    }
                                } else {
                                    view.error =
                                        Some("会话重命名未完成，请刷新目录后重试。".into());
                                }
                                view.refresh_assistant_directory(false, window, cx);
                                cx.notify();
                            });
                        })
                        .detach();
                    });
                    false
                })
        });
    }
}
