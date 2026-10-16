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

    pub(super) fn dispatch_text(
        self,
        menu_focus: Option<&gpui_kit::FocusHandle>,
        scope: &gpui_kit::FocusHandle,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        // An unavailable text action is still a text command, never a graph fallback.
        if window.is_action_available(&input::SelectAll, cx) {
            let action = self.text_action();
            if window.is_action_available(action.as_ref(), cx) {
                window.dispatch_action(action, cx);
            }
            return true;
        }
        let Some(focus) = menu_focus.filter(|_| {
            window
                .context_stack()
                .iter()
                .any(|context| context.contains("AppMenuBar"))
        }) else {
            return false;
        };
        let focus = focus.clone();
        let scope = scope.clone();
        // AppMenuBar's queued dismissal otherwise restores the focused menu trigger
        // after this command. Finish the effect cycle before restoring the text target.
        window.defer(cx, move |window, cx| {
            // A dismissed field must never redirect a captured text command to the graph.
            if scope.contains(&focus, window) {
                focus.focus(window, cx);
                focus.dispatch_action(self.text_action().as_ref(), window, cx);
            }
        });
        true
    }
}

impl From<EditCommand> for canvas::GraphEditCommand {
    fn from(command: EditCommand) -> Self {
        match command {
            EditCommand::Undo => Self::Undo,
            EditCommand::Redo => Self::Redo,
            EditCommand::Cut => Self::Cut,
            EditCommand::Copy => Self::Copy,
            EditCommand::Paste => Self::Paste,
            EditCommand::Delete => Self::Delete,
        }
    }
}

impl Workbench {
    pub(super) fn edit_menu_state(
        &self,
        window: &Window,
        cx: &App,
    ) -> (Option<gpui_kit::FocusHandle>, [bool; 6]) {
        if window.is_action_available(&input::SelectAll, cx) {
            return (
                window.focused(cx),
                [
                    EditCommand::Undo,
                    EditCommand::Redo,
                    EditCommand::Cut,
                    EditCommand::Copy,
                    EditCommand::Paste,
                    EditCommand::Delete,
                ]
                .map(|command| window.is_action_available(command.text_action().as_ref(), cx)),
            );
        }
        let Some(panel) = self.editor_command_panel(window, cx) else {
            return (None, [false; 6]);
        };
        let editor = panel.view();
        if let Ok(graph) = editor.clone().downcast::<canvas::GraphCanvas>() {
            use canvas::GraphEditCommand::*;
            return (
                None,
                [Undo, Redo, Cut, Copy, Paste, Delete]
                    .map(|command| graph.read(cx).is_edit_command_enabled(command)),
            );
        }
        if let Ok(document) = editor
            .clone()
            .downcast::<crate::documents::DocumentEditor>()
        {
            return (None, [document.read(cx).is_editing(); 6]);
        }
        (
            None,
            [
                false,
                false,
                false,
                editor.entity_type() == std::any::TypeId::of::<crate::databases::DatabaseEditor>(),
                false,
                editor.entity_type() == std::any::TypeId::of::<crate::minds::MindCanvas>(),
            ],
        )
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
        let menu_focus = self
            .menu_context
            .as_ref()
            .and_then(|context| context.text_focus.as_ref());
        if command.dispatch_text(menu_focus, &self.focus, window, cx) {
            return;
        }
        let Some(panel) = self.editor_command_panel(window, cx) else {
            return;
        };
        let editor = panel.view();
        if let Ok(graph) = editor.clone().downcast::<canvas::GraphCanvas>() {
            graph.update(cx, |graph, cx| graph.edit(command.into(), cx));
            return;
        }
        let action: Box<dyn Action> = if editor
            .clone()
            .downcast::<crate::documents::DocumentEditor>()
            .is_ok_and(|document| document.read(cx).is_editing())
        {
            command.text_action()
        } else if editor.entity_type() == std::any::TypeId::of::<crate::minds::MindCanvas>()
            && command == EditCommand::Delete
        {
            Box::new(DeleteTopics)
        } else if editor.entity_type() == std::any::TypeId::of::<crate::databases::DatabaseEditor>()
            && command == EditCommand::Copy
        {
            Box::new(CopyDatabaseSelection)
        } else {
            return;
        };
        panel
            .focus_handle(cx)
            .dispatch_action(action.as_ref(), window, cx);
    }
}
