//! Menu editing reuses the selected editor's actions and the component text engine.
use super::super::Workbench;
use crate::{canvas, databases::CopyDatabaseSelection, minds::DeleteTopics};
use gpui_kit::component::{
    dock::{BasePanelView, DockPlacement},
    input,
};
use gpui_kit::{Action, App, Context, Window};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq)]
pub(in crate::workbench) enum EditCommand {
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Delete,
}

impl EditCommand {
    pub(in crate::workbench) fn text_action(self) -> Box<dyn Action> {
        match self {
            Self::Undo => Box::new(input::Undo),
            Self::Redo => Box::new(input::Redo),
            Self::Cut => Box::new(input::Cut),
            Self::Copy => Box::new(input::Copy),
            Self::Paste => Box::new(input::Paste),
            Self::Delete => Box::new(input::Delete),
        }
    }
}

impl Workbench {
    pub(super) fn edit_menu_availability(&self, cx: &App) -> [bool; 6] {
        if self.active_editor_panel(cx).is_none() {
            return [false; 6];
        }
        let details = self.details.read(cx);
        if let Some(graph) = details.graph() {
            let graph = graph.read(cx);
            return [
                graph.graph.editing.can_undo,
                graph.graph.editing.can_redo,
                false,
                false,
                false,
                true,
            ];
        }
        if let Some(document) = details.document() {
            return [document.read(cx).is_editing(); 6];
        }
        [
            false,
            false,
            false,
            details.database().is_some(),
            false,
            details.mind().is_some(),
        ]
    }

    pub(in crate::workbench) fn editor_command_panel(
        &self,
        window: &Window,
        cx: &App,
    ) -> Option<Arc<dyn BasePanelView>> {
        let editor = self.active_editor_panel(cx)?;
        let dock = self.dock.read(cx);
        let focused_elsewhere = [
            DockPlacement::Center,
            DockPlacement::Left,
            DockPlacement::Right,
            DockPlacement::Bottom,
        ]
        .into_iter()
        .filter_map(|placement| dock.layout(placement))
        .flat_map(|tree| tree.panels())
        .filter_map(|id| dock.panel(id))
        .any(|panel| {
            panel.panel_id(cx) != editor.panel_id(cx)
                && panel.panel_id(cx) != self.details.entity_id().into()
                && panel.view().entity_type()
                    != std::any::TypeId::of::<super::super::activity::ActivityPanel>()
                && panel.focus_handle(cx).contains_focused(window, cx)
        });
        // A result, conversation or plugin may be focused beside a still-visible editor.
        // Details and navigation can address that editor; unrelated panels cannot.
        (!focused_elsewhere).then_some(editor)
    }

    pub(super) fn edit_current(
        &self,
        command: EditCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text_action = command.text_action();
        // AppMenuBar restores the pre-menu focus. Property inputs keep their own editing
        // semantics, so a text operation must not accidentally delete graph nodes.
        if window.is_action_available(text_action.as_ref(), cx) {
            window.dispatch_action(text_action, cx);
            return;
        }
        let Some(panel) = self.editor_command_panel(window, cx) else {
            return;
        };
        let details = self.details.read(cx);
        let action: Box<dyn Action> = if details.graph().is_some() {
            match command {
                EditCommand::Undo => Box::new(canvas::UndoGraph),
                EditCommand::Redo => Box::new(canvas::RedoGraph),
                EditCommand::Delete => Box::new(canvas::DeleteSelection),
                _ => return,
            }
        } else if details
            .document()
            .is_some_and(|document| document.read(cx).is_editing())
        {
            text_action
        } else if details.mind().is_some() && command == EditCommand::Delete {
            Box::new(DeleteTopics)
        } else if details.database().is_some() && command == EditCommand::Copy {
            Box::new(CopyDatabaseSelection)
        } else {
            return;
        };
        panel
            .focus_handle(cx)
            .dispatch_action(action.as_ref(), window, cx);
    }
}
