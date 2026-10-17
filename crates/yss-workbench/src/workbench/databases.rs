//! Data routing consumes the current index and revision-bound Application queries.
use super::Workbench;
use super::details::DetailsTarget;
use crate::databases::{DatabaseEditor, DatabaseEvent, query};
use gpui_kit::component::dock::{DockPlacement, panel_handle};
use gpui_kit::{AppContext, Context, Window};
use yss_ui::lifecycle::LogReleased;
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
        if let Some(editor) = self
            .databases
            .get(&id)
            .and_then(gpui_kit::WeakEntity::upgrade)
        {
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
        let page_size = crate::preferences::current(cx).tables.page_size;
        let task = self.services.run(move |services| {
            query::read(
                &services.application,
                identity,
                read_entry.id,
                read_entry.revision,
                0,
                page_size,
                None,
            )
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = task.await.ok().and_then(Result::ok);
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
                if intent
                    .as_deref()
                    .is_some_and(|id| !view.is_current_intent(id))
                {
                    return;
                }
                if page_size != crate::preferences::current(cx).tables.page_size {
                    view.open_database(entry.id, intent, window, cx);
                    return;
                }
                if view.project.as_ref().is_none_or(|project| {
                    !project
                        .index
                        .databases
                        .iter()
                        .any(|current| current.id == entry.id && current.revision == entry.revision)
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
            })
            .log_released();
        })
        .detach();
    }
    pub(super) fn install_database(
        &mut self,
        entry: yss_project::ProjectDatabaseIndexEntry,
        read: Option<query::DatabaseRead>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui_kit::Entity<DatabaseEditor> {
        let project = self
            .project
            .as_ref()
            .expect("installed project")
            .identity
            .clone();
        let id = entry.id.clone();
        let services = self.services.tasks.clone();
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
        let mut status = (
            editor.read(cx).dirty(),
            editor.read(cx).busy(),
            editor.read(cx).name().to_owned(),
        );
        self.subscriptions
            .push(cx.observe(&editor, move |_, editor, cx| {
                let editor = editor.read(cx);
                if status.0 != editor.dirty()
                    || status.1 != editor.busy()
                    || status.2 != editor.name()
                {
                    status = (editor.dirty(), editor.busy(), editor.name().to_owned());
                    cx.notify();
                }
            }));
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
                        view.activate_editor(DetailsTarget::Database(editor.downgrade()), cx)
                    }
                    DatabaseEvent::SessionChanged => {
                        view.rebind_session(window, cx);
                    }
                }
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
}
