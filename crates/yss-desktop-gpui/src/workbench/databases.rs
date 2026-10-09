//! Data routing consumes the current index and revision-bound Application queries.
use super::Workbench;
use crate::databases::{DatabaseEditor, DatabaseEvent, query};
use gpui::{AppContext, Context, Window};
use gpui_component::dock::{DockPlacement, panel_handle};
impl Workbench {
    pub(super) fn open_database(
        &mut self,
        id: String,
        intent: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = &self.project else {
            return;
        };
        if self.busy || self.closing {
            return;
        }
        if let Some(editor) = self.databases.get(&id).and_then(gpui::WeakEntity::upgrade) {
            self.present_panel(
                panel_handle(editor.clone()),
                DockPlacement::Center,
                window,
                cx,
            );
            editor.update(cx, |editor, cx| {
                if editor.read_failed() {
                    editor.reload(true, window, cx);
                }
                editor.focus_table(window, cx);
            });
            if let Some(intent) = intent {
                self.finish_intent(&intent, true, window, cx);
            }
            return;
        }
        let Some(entry) = project
            .index
            .databases
            .iter()
            .find(|entry| entry.id == id)
            .cloned()
        else {
            if let Some(intent) = intent {
                self.finish_intent(&intent, false, window, cx);
            }
            return;
        };
        let key = format!("database:{id}");
        if !self.opening.insert(key.clone()) {
            if let Some(intent) = intent {
                self.finish_intent(&intent, false, window, cx);
            }
            return;
        }
        let identity = project.identity.clone();
        let expected = identity.clone();
        let lifecycle = self.lifecycle;
        let read_entry = entry.clone();
        let task = self.services.run(move |services| {
            query::read(
                services,
                identity,
                read_entry.id,
                read_entry.revision,
                0,
                None,
            )
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = task.await.ok().and_then(Result::ok);
            let _ =
                view.update_in(cx, |view, window, cx| {
                    if view.lifecycle != lifecycle
                        || view
                            .project
                            .as_ref()
                            .is_none_or(|project| project.identity != expected)
                    {
                        return;
                    }
                    view.opening.remove(&key);
                    if view.project.as_ref().is_none_or(|project| {
                        !project.index.databases.iter().any(|current| {
                            current.id == entry.id && current.revision == entry.revision
                        })
                    }) {
                        view.open_database(entry.id, intent, window, cx);
                        return;
                    }
                    let applied = result.is_some();
                    view.install_database(entry, result, window, cx)
                        .update(cx, |editor, cx| editor.focus_table(window, cx));
                    if let Some(intent) = intent {
                        view.finish_intent(&intent, applied, window, cx);
                    }
                    cx.notify();
                });
        })
        .detach();
    }
    pub(super) fn install_database(
        &mut self,
        entry: yss_project::ProjectDatabaseIndexEntry,
        read: Option<query::DatabaseRead>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Entity<DatabaseEditor> {
        let project = self
            .project
            .as_ref()
            .expect("installed project")
            .identity
            .clone();
        let id = entry.id.clone();
        let services = self.services.clone();
        let editor = cx.new(|cx| {
            DatabaseEditor::new(
                services,
                project,
                entry.id,
                entry.revision,
                entry.name.unwrap_or_else(|| id.clone()),
                window,
                cx,
            )
        });
        editor.update(cx, |editor, cx| match read {
            Some(read) => editor.install_read(read, window, cx),
            None => editor.fail_read(cx),
        });
        let mut read_failed = editor.read(cx).read_failed();
        self.subscriptions.push(cx.subscribe_in(
            &editor,
            window,
            move |view, editor, event, window, cx| {
                let next = editor.read(cx).read_failed();
                if next != read_failed {
                    read_failed = next;
                    view.refresh_resource_rows(cx);
                }
                match event {
                    DatabaseEvent::Activated => {
                        view.details.update(cx, |details, cx| {
                            details.set_database(editor.downgrade(), cx)
                        });
                        view.activate_file(editor.read(cx).id.clone(), cx);
                    }
                    DatabaseEvent::SessionChanged => {
                        view.rebind_session(window, cx);
                    }
                    DatabaseEvent::Changed => {}
                }
                view.details.update(cx, |_, cx| cx.notify());
                cx.notify();
            },
        ));
        self.databases.insert(id, editor.downgrade());
        self.refresh_resource_rows(cx);
        self.present_panel(
            panel_handle(editor.clone()),
            DockPlacement::Center,
            window,
            cx,
        );
        editor
    }
    pub(super) fn refresh_databases(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(project) = &self.project else {
            return;
        };
        for (id, editor) in &self.databases {
            let Some(editor) = editor.upgrade() else {
                continue;
            };
            if let Some(entry) = project.index.databases.iter().find(|entry| &entry.id == id) {
                editor.update(cx, |editor, cx| {
                    editor.replace_index(
                        entry.revision,
                        entry.name.clone().unwrap_or_else(|| id.clone()),
                        window,
                        cx,
                    )
                });
            } else {
                editor.update(cx, |editor, cx| editor.unavailable(cx));
            }
        }
    }
}
