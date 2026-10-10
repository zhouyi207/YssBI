//! Lock and update the original editor entity without replacing DockArea placement.
use super::*;
use crate::{documents::DocumentEditor, file_commands::FileSaveOutcome, minds::MindCanvas};
use gpui_kit::App;

pub(super) enum FilePanel {
    Document(Entity<DocumentEditor>),
    Mind(Entity<MindCanvas>),
}

impl FilePanel {
    pub(super) fn version(&self, cx: &App) -> FileVersion {
        match self {
            Self::Document(panel) => panel.read(cx).snapshot.version.clone(),
            Self::Mind(panel) => panel.read(cx).snapshot.version.clone(),
        }
    }
    pub(super) fn dirty(&self, cx: &App) -> bool {
        match self {
            Self::Document(panel) => panel.read(cx).dirty(),
            Self::Mind(panel) => panel.read(cx).dirty(),
        }
    }
    pub(super) fn lock(&self, cx: &mut App) -> bool {
        match self {
            Self::Document(panel) => panel.update(cx, |panel, cx| panel.prepare_save(cx).is_some()),
            Self::Mind(panel) => panel.update(cx, |panel, cx| panel.prepare_save(cx).is_some()),
        }
    }
    pub(super) fn unlock(&self, window: &mut Window, cx: &mut App) {
        match self {
            Self::Document(panel) => {
                panel.update(cx, |panel, cx| panel.cancel_prepared_save(window, cx))
            }
            Self::Mind(panel) => {
                panel.update(cx, |panel, cx| panel.cancel_prepared_save(window, cx))
            }
        }
    }
}

impl Workbench {
    pub(super) fn file_panel(&self, kind: AuthoredKind, path: &str) -> Option<FilePanel> {
        match kind {
            AuthoredKind::Document => self
                .documents
                .get(path)
                .and_then(gpui_kit::WeakEntity::upgrade)
                .map(FilePanel::Document),
            AuthoredKind::Mind => self
                .minds
                .get(path)
                .and_then(gpui_kit::WeakEntity::upgrade)
                .map(FilePanel::Mind),
            AuthoredKind::Chart => None,
        }
    }

    pub(super) fn accept_file_operation(
        &mut self,
        target: &FileTarget,
        action: ResourceAction,
        panel: Option<FilePanel>,
        result: FileResult,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(action, ResourceAction::Delete) {
            self.retire_file(target, panel, window, cx);
            return;
        }
        if matches!(action, ResourceAction::Duplicate)
            && let Some(panel) = &panel
        {
            panel.unlock(window, cx);
        }
        match result {
            FileResult::Document(Some(snapshot)) => {
                let path = snapshot.path.as_str().to_owned();
                if matches!(action, ResourceAction::Rename)
                    && let Some(FilePanel::Document(panel)) = panel
                {
                    self.documents.remove(&target.path);
                    panel.update(cx, |panel, cx| {
                        panel.finish_save(
                            FileSaveOutcome {
                                snapshot: Some(snapshot),
                                failed: false,
                            },
                            window,
                            cx,
                        )
                    });
                    self.documents.insert(path, panel.downgrade());
                    panel.update(cx, |panel, cx| panel.focus_editor(window, cx));
                } else {
                    self.install_document(snapshot, window, cx)
                        .update(cx, |panel, cx| panel.focus_editor(window, cx));
                }
            }
            FileResult::Mind(Some(snapshot)) => {
                let path = snapshot.path.as_str().to_owned();
                if matches!(action, ResourceAction::Rename)
                    && let Some(FilePanel::Mind(panel)) = panel
                {
                    self.minds.remove(&target.path);
                    panel.update(cx, |panel, cx| {
                        panel.finish_save(
                            FileSaveOutcome {
                                snapshot: Some(snapshot),
                                failed: false,
                            },
                            window,
                            cx,
                        )
                    });
                    self.minds.insert(path, panel.downgrade());
                    panel.update(cx, |panel, cx| panel.focus_canvas(window, cx));
                } else {
                    self.install_mind(snapshot, window, cx)
                        .update(cx, |panel, cx| panel.focus_canvas(window, cx));
                }
            }
            _ => {
                if let Some(panel) = panel {
                    panel.unlock(window, cx);
                }
                self.error = Some(crate::text::translate(
                    "native.workbench.resourceCommittedUnavailable",
                ));
            }
        }
    }

    fn retire_file(
        &mut self,
        target: &FileTarget,
        panel: Option<FilePanel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match target.kind {
            AuthoredKind::Document => {
                self.documents.remove(&target.path);
            }
            AuthoredKind::Mind => {
                self.minds.remove(&target.path);
            }
            AuthoredKind::Chart => {}
        }
        match panel {
            Some(FilePanel::Document(panel)) => {
                if self
                    .details
                    .read(cx)
                    .document()
                    .is_some_and(|current| current.entity_id() == panel.entity_id())
                {
                    self.details.update(cx, |details, cx| details.clear(cx));
                }
                self.dock
                    .update(cx, |dock, cx| dock.remove_panel(panel, window, cx));
            }
            Some(FilePanel::Mind(panel)) => {
                if self
                    .details
                    .read(cx)
                    .mind()
                    .is_some_and(|current| current.entity_id() == panel.entity_id())
                {
                    self.details.update(cx, |details, cx| details.clear(cx));
                }
                self.dock
                    .update(cx, |dock, cx| dock.remove_panel(panel, window, cx));
            }
            None => {}
        }
    }
}
