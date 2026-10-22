//! Shell-only Dock placement, navigation and UI-intent routing for the retained assistant owner.
mod navigation;

use super::{
    Workbench,
    activity::{ActivityEvent, ActivityPanel, ReadState},
};
use gpui_kit::component::dock::{DockPlacement, PaneRef, panel_handle};
use gpui_kit::{AppContext, Context, Window};
use yss_application::activity_panel::ActivityPanelDocument;
use yss_assistant_ui::{AssistantEventKind, ConversationPanel, DirectoryReadState};

impl Workbench {
    pub(super) fn connect_assistant(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.assistant_subscription = Some(cx.subscribe_in(
            &self.assistant,
            window,
            |view, _, event, window, cx| {
                let current = view.project.as_ref().map(|project| &project.identity);
                if event.project() != current {
                    if matches!(
                        event.kind(),
                        AssistantEventKind::OpenResource(_) | AssistantEventKind::OpenResult(_)
                    ) {
                        view.error =
                            Some(yss_ui::text::t("panel.assistantResourceUnavailable").into());
                        cx.notify();
                    }
                    return;
                }
                match event.kind() {
                    AssistantEventKind::DirectoryChanged => {
                        view.sync_assistant_directory(window, cx)
                    }
                    AssistantEventKind::PresentConversation(panel) => {
                        view.present_panel(
                            panel_handle(panel.clone()),
                            DockPlacement::Center,
                            window,
                            cx,
                        );
                        view.sync_active_conversation(cx);
                    }
                    AssistantEventKind::RemoveConversation(panel) => {
                        view.dock
                            .update(cx, |dock, cx| dock.remove_panel(panel.clone(), window, cx));
                        view.sync_active_conversation(cx);
                    }
                    AssistantEventKind::PresentDirectory => {
                        view.ensure_assistant_directory(window, cx);
                        view.present_assistant(window, cx);
                    }
                    AssistantEventKind::MetadataChanged => {
                        view.dock.update(cx, |_, cx| cx.notify())
                    }
                    AssistantEventKind::OpenResource(resource) => {
                        view.open_project_resource(resource, window, cx)
                    }
                    AssistantEventKind::OpenResult(reference) => {
                        view.open_result(*reference, None, None, window, cx)
                    }
                    AssistantEventKind::Settings => view.show_settings(window, cx),
                    AssistantEventKind::Error(message) => view.error = message.clone(),
                }
                cx.notify();
            },
        ));
        self.assistant_observation = Some(cx.observe(&self.assistant, |_, _, cx| cx.notify()));
    }

    pub(super) fn show_assistant(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.project.is_none() || self.busy || self.closing {
            return false;
        }
        self.ensure_assistant_directory(window, cx);
        self.present_assistant(window, cx);
        if let Some(intent) = self.assistant_intent.take() {
            self.finish_intent(&intent, true, window, cx);
        }
        self.refresh_assistant_directory(window, cx);
        true
    }

    pub(super) fn refresh_assistant_directory(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ensure_assistant_directory(window, cx);
        self.assistant
            .update(cx, |assistant, cx| assistant.refresh_directory(window, cx));
    }

    pub(super) fn install_assistant_document(
        &mut self,
        document: ActivityPanelDocument,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.assistant.update(cx, |assistant, cx| {
            assistant.install_restored_directory(document, cx)
        });
        self.sync_assistant_directory(window, cx);
    }

    fn sync_assistant_directory(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let assistant = self.assistant.read(cx);
        let document = assistant.directory().cloned();
        let state = match assistant.read_state() {
            DirectoryReadState::Loading => ReadState::Loading,
            DirectoryReadState::Ready => ReadState::Ready,
            DirectoryReadState::Failed(key) => ReadState::Failed(key),
        };
        let panel = self.ensure_assistant_directory(window, cx);
        panel.update(cx, |panel, cx| {
            if let Some(document) = document {
                panel.replace_document(document, cx);
            }
            panel.set_read_state(state, cx);
        });
        self.sync_active_conversation(cx);
    }

    fn ensure_assistant_directory(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui_kit::Entity<ActivityPanel> {
        if let Some(panel) = self
            .activities
            .get("assistant")
            .and_then(gpui_kit::WeakEntity::upgrade)
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
                ActivityEvent::DeleteConversation(id, title) => {
                    view.delete_conversation(id.clone(), title.clone(), window, cx)
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
            // Background mounting preserves selection; explicit show requests select afterwards.
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
        if self.project.is_none() {
            return;
        }
        let Some(panel) = self
            .activities
            .get("assistant")
            .and_then(gpui_kit::WeakEntity::upgrade)
        else {
            return;
        };
        self.present_panel(panel_handle(panel), DockPlacement::Left, window, cx);
    }

    pub(super) fn new_conversation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_closing(cx) {
            return;
        }
        self.assistant
            .update(cx, |assistant, cx| assistant.new_conversation(window, cx));
    }

    pub(super) fn open_conversation(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || self.closing || self.assistant.read(cx).is_busy() {
            return;
        }
        self.assistant.update(cx, |assistant, cx| {
            assistant.open_conversation(id, window, cx)
        });
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
        self.assistant.update(cx, |assistant, cx| {
            assistant.rename_conversation(id, title, window, cx)
        });
    }

    pub(super) fn delete_conversation(
        &mut self,
        id: String,
        title: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_closing(cx) {
            return;
        }
        self.assistant.update(cx, |assistant, cx| {
            assistant.delete_conversation(id, title, window, cx)
        });
    }

    fn visible_conversation(
        &self,
        cx: &gpui_kit::App,
    ) -> Option<gpui_kit::Entity<ConversationPanel>> {
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
        if self.busy
            || self.closing
            || self.assistant.read(cx).is_busy()
            || self.dock.read(cx).is_locked()
        {
            return;
        }
        if let Some(panel) = self.visible_conversation(cx)
            && panel.read(cx).session_id().as_str() == id
        {
            self.assistant
                .update(cx, |assistant, cx| assistant.note_closed(Some(id), cx));
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
            .map(|panel| panel.read(cx).session_id().to_string());
        self.mark_activity("assistant", active.as_deref(), cx);
    }
}
