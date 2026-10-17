//! A chart save submits the captured configuration through the shared file session.
use super::ChartEditor;
use gpui_kit::{Context, Window};
use yss_chart_document::ChartDocument;
use yss_project_model::chart::ChartEdit;
use yss_resource_ui::file_session::{FileEditor, FileSaveOutcome, FileSaveRequest, FileSession};

pub type ChartSaveRequest = FileSaveRequest<ChartDocument>;
pub type ChartSaveOutcome = FileSaveOutcome<ChartDocument>;

impl FileEditor for ChartEditor {
    type File = ChartDocument;
    const PANEL_NAME: &'static str = "chart-editor";
    const PATH_KEY: &'static str = "chartPath";

    fn session(&self) -> &FileSession<ChartDocument> {
        &self.file
    }

    fn session_mut(&mut self) -> &mut FileSession<ChartDocument> {
        &mut self.file
    }

    fn has_draft(&self) -> bool {
        self.draft != self.file.snapshot().content
    }

    fn draft_edits(&mut self, _: &mut Context<Self>) -> Option<Vec<ChartEdit>> {
        Some(if self.has_draft() {
            vec![ChartEdit::SetDocument {
                document: self.draft.clone(),
            }]
        } else {
            vec![]
        })
    }

    fn snapshot_replaced(&mut self, keep_draft: bool, window: &mut Window, cx: &mut Context<Self>) {
        if keep_draft || !self.has_draft() {
            return;
        }
        self.draft = self.file.snapshot().content.clone();
        self.draft_epoch = self.draft_epoch.wrapping_add(1);
        self.schedule_preview(window, cx);
    }
}

impl ChartEditor {
    /// Re-reads the configuration and queries the preview data again.
    pub fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.refresh(window, cx);
        if !self.file.removed() {
            self.schedule_preview(window, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{AppContext, TestAppContext, WindowOptions};
    use std::sync::Arc;
    use yss_application::session::ApplicationState;
    use yss_chart_document::ChartType;
    use yss_desktop_services::{ApplicationTasks, DesktopEvents};
    use yss_project::file_resources::FileCommand;
    use yss_project_identity::OperationId;
    use yss_project_model::file::FileContent;

    #[gpui_kit::test]
    async fn chart_save_writes_the_draft_and_a_reread_keeps_it_clean(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        yss_project::fixtures::write_project(
            &yss_project_model::ProjectData::new(),
            directory.path().to_str().unwrap(),
        )
        .unwrap();
        let application = ApplicationState::initialize().unwrap();
        application
            .load_project_for_application(directory.path().join("metadata.yssbi").to_str().unwrap())
            .unwrap();
        let identity = application
            .capture_session()
            .unwrap()
            .project_instance_id()
            .clone();
        let created = application
            .apply_file_command::<ChartDocument>(
                identity.clone(),
                OperationId::new(),
                FileCommand::Create {
                    name: "Sales".into(),
                },
            )
            .unwrap()
            .snapshot
            .unwrap();
        let catalog = Arc::new(application.query_project_index(identity).unwrap());
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let tasks = Arc::new(ApplicationTasks::new(
            application,
            runtime.handle().clone(),
            DesktopEvents::default(),
        ));
        cx.executor().allow_parking();
        let (window, editor) = cx.update(|cx| {
            gpui_kit::init(cx);
            yss_ui::init(cx);
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| ChartEditor::new(tasks, created, catalog, window, cx))
            })
            .unwrap()
        });
        cx.update_window(window, |_, window, cx| {
            editor.update(cx, |editor, cx| {
                let epoch = editor.draft_epoch;
                editor.update_draft(
                    epoch,
                    |draft| draft.chart_type = ChartType::Line,
                    window,
                    cx,
                );
                assert!(editor.dirty());
                editor.save(window, cx);
            })
        })
        .unwrap();
        cx.condition(&editor, |editor, _| !editor.busy()).await;
        cx.update(|cx| assert!(!editor.read(cx).dirty()));
        let disk = std::fs::read(directory.path().join("charts/Sales.yssbi-chart")).unwrap();
        assert_eq!(
            ChartDocument::decode(&disk).unwrap().chart_type,
            ChartType::Line
        );

        cx.update_window(window, |_, window, cx| {
            editor.update(cx, |editor, cx| editor.refresh(window, cx))
        })
        .unwrap();
        cx.condition(&editor, |editor, _| !editor.busy()).await;
        cx.update(|cx| {
            let editor = editor.read(cx);
            assert_eq!(editor.draft.chart_type, ChartType::Line);
            assert!(!editor.dirty());
        });
    }
}
