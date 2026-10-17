//! Menu editing reuses the selected editor's actions and the component text engine.
use super::super::{EditorPanel, Workbench, files::FilePanel};
use super::{EditorCommands, EditorKind};
use crate::{
    canvas,
    databases::{CopyDatabaseSelection, SelectDatabasePage},
    documents::ToggleDocumentPreview,
    minds::{DeleteTopics, FitMind, FitTopics, SelectTopics},
    workbench::dock::{CloseAllTabs, CloseOtherTabs, CloseTab},
};
use gpui_kit::component::{
    dock::{BasePanelView, DockPlacement},
    input,
};
use gpui_kit::{Action, App, Context, FocusHandle, Window};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq)]
pub(in crate::workbench) enum EditCommand {
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Delete,
    SelectAll,
}

impl EditCommand {
    pub(super) const ALL: [Self; 7] = [
        Self::Undo,
        Self::Redo,
        Self::Cut,
        Self::Copy,
        Self::Paste,
        Self::Delete,
        Self::SelectAll,
    ];

    pub(in crate::workbench) fn text_action(self) -> Box<dyn Action> {
        match self {
            Self::Undo => Box::new(input::Undo),
            Self::Redo => Box::new(input::Redo),
            Self::Cut => Box::new(input::Cut),
            Self::Copy => Box::new(input::Copy),
            Self::Paste => Box::new(input::Paste),
            Self::Delete => Box::new(input::Delete),
            Self::SelectAll => Box::new(input::SelectAll),
        }
    }

    fn graph_command(self) -> Option<canvas::GraphEditCommand> {
        Some(match self {
            Self::Undo => canvas::GraphEditCommand::Undo,
            Self::Redo => canvas::GraphEditCommand::Redo,
            Self::Cut => canvas::GraphEditCommand::Cut,
            Self::Copy => canvas::GraphEditCommand::Copy,
            Self::Paste => canvas::GraphEditCommand::Paste,
            Self::Delete => canvas::GraphEditCommand::Delete,
            Self::SelectAll => return None,
        })
    }

    /// The editor's own action for this command; a menu item carrying it shows the
    /// shortcut that is effective inside the editor.
    pub(super) fn editor_action(self, kind: EditorKind) -> Option<Box<dyn Action>> {
        Some(match (kind, self) {
            (EditorKind::Graph, Self::Undo) => Box::new(canvas::UndoGraph),
            (EditorKind::Graph, Self::Redo) => Box::new(canvas::RedoGraph),
            (EditorKind::Graph, Self::Cut) => Box::new(input::Cut),
            (EditorKind::Graph, Self::Copy) => Box::new(input::Copy),
            (EditorKind::Graph, Self::Paste) => Box::new(input::Paste),
            (EditorKind::Graph, Self::Delete) => Box::new(canvas::DeleteSelection),
            (EditorKind::Graph, Self::SelectAll) => Box::new(canvas::SelectAll),
            (EditorKind::Mind, Self::Delete) => Box::new(DeleteTopics),
            (EditorKind::Mind, Self::SelectAll) => Box::new(SelectTopics),
            (EditorKind::Database, Self::Copy) => Box::new(CopyDatabaseSelection),
            (EditorKind::Database, Self::SelectAll) => Box::new(SelectDatabasePage),
            _ => return None,
        })
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

impl EditorPanel {
    fn can_edit(&self, command: EditCommand, cx: &App) -> bool {
        match self {
            Self::Graph(graph) => command
                .graph_command()
                .is_none_or(|command| graph.read(cx).is_edit_command_enabled(command)),
            Self::File(FilePanel::Document(document)) => document.read(cx).is_editing(),
            Self::File(FilePanel::Mind(_)) => {
                matches!(command, EditCommand::Delete | EditCommand::SelectAll)
            }
            Self::File(FilePanel::Chart(_)) => false,
            Self::Database(_) => matches!(command, EditCommand::Copy | EditCommand::SelectAll),
        }
    }

    fn edit(&self, command: EditCommand, focus: &FocusHandle, window: &mut Window, cx: &mut App) {
        if !self.can_edit(command, cx) {
            return;
        }
        let action = match self {
            Self::File(FilePanel::Document(_)) => Some(command.text_action()),
            _ => command.editor_action(self.kind()),
        };
        if let Some(action) = action {
            focus.dispatch_action(action.as_ref(), window, cx);
        }
    }

    fn kind(&self) -> EditorKind {
        match self {
            Self::Graph(_) => EditorKind::Graph,
            Self::File(FilePanel::Document(_)) => EditorKind::Document,
            Self::File(FilePanel::Mind(_)) => EditorKind::Mind,
            Self::File(FilePanel::Chart(_)) => EditorKind::Chart,
            Self::Database(_) => EditorKind::Database,
        }
    }

    pub(super) fn menu_commands(&self, cx: &App) -> EditorCommands {
        EditorCommands {
            kind: Some(self.kind()),
            graph: match self {
                Self::Graph(graph) => graph.read(cx).command_state(cx),
                _ => Default::default(),
            },
            export: matches!(self, Self::Database(editor) if editor.read(cx).can_export()),
        }
    }

    /// The editor's own actions that the application menu offers.
    fn handles(&self, action: &dyn Action) -> bool {
        let actions: &[&dyn Action] = match self {
            Self::Graph(_) => &[
                &canvas::DuplicateSelection,
                &canvas::OpenNodePalette,
                &canvas::FrameGraph,
                &canvas::FrameSelection,
                &canvas::RunCurrentNode,
                &canvas::RunToNode,
                &canvas::RefreshRunState,
            ],
            Self::File(FilePanel::Mind(_)) => &[&FitMind, &FitTopics],
            Self::File(FilePanel::Document(_)) => &[&ToggleDocumentPreview],
            Self::File(FilePanel::Chart(_)) | Self::Database(_) => &[],
        };
        actions.iter().any(|handled| handled.partial_eq(action))
    }
}

impl Workbench {
    pub(super) fn edit_menu_state(
        &self,
        window: &Window,
        cx: &App,
    ) -> (Option<FocusHandle>, [bool; 7]) {
        if window.is_action_available(&input::SelectAll, cx) {
            return (
                window.focused(cx),
                EditCommand::ALL
                    .map(|command| window.is_action_available(command.text_action().as_ref(), cx)),
            );
        }
        let Some(editor) = self
            .editor_command_panel(window, cx)
            .and_then(|panel| EditorPanel::from_view(panel.view()))
        else {
            return (None, [false; 7]);
        };
        (
            None,
            EditCommand::ALL.map(|command| editor.can_edit(command, cx)),
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
        if let Some(editor) = EditorPanel::from_view(panel.view()) {
            editor.edit(command, &panel.focus_handle(cx), window, cx);
        }
    }
}

impl Workbench {
    /// Menu items carry the editor's own actions so they show its shortcuts. Focus elsewhere
    /// leaves such an action unhandled; it then reaches the workbench and goes to the editor.
    pub(in crate::workbench) fn forward_to_editor(
        &self,
        action: &dyn Action,
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.is_closing(cx) {
            return;
        }
        let Some(panel) = self.editor_command_panel(window, cx) else {
            return;
        };
        let focus = panel.focus_handle(cx);
        // An action that already passed through the editor must not be sent back into it.
        if focus.contains_focused(window, cx) {
            return;
        }
        let tab_action = [&CloseTab as &dyn Action, &CloseOtherTabs, &CloseAllTabs]
            .into_iter()
            .any(|tab_action| tab_action.partial_eq(action));
        // An action nothing below handles would bubble back here and be forwarded again.
        let handled = if tab_action {
            self.panel_placement(panel.panel_id(cx), cx) == Some(DockPlacement::Center)
        } else {
            EditorPanel::from_view(panel.view()).is_some_and(|editor| editor.handles(action))
        };
        if handled {
            let action = action.boxed_clone();
            // Closing tabs and editor commands may update the workbench, which is leased here.
            window.defer(cx, move |window, cx| {
                focus.dispatch_action(action.as_ref(), window, cx);
            });
        }
    }
}
