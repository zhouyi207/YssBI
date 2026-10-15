use super::{ChartEditor, ChartRead};
use gpui_kit::{AppContext, TestAppContext, WindowOptions};
use std::sync::Arc;
use yss_application::session::{ApplicationSessionSlot, ApplicationState, NodeComponents};
use yss_chart_document::{ChartDocument, ChartResourcePath, ChartType};
use yss_desktop_services::{ApplicationTasks, DesktopEvents};
use yss_project::{ProjectChartIndexEntry, ProjectDatabaseIndexEntry};
use yss_project_identity::{ProjectResourcePath, ResourceRevision};

#[gpui_kit::test]
fn chart_callbacks_track_dataset_dependencies_not_unrelated_publications(cx: &mut TestAppContext) {
    let fixture = yss_project::fixtures::TempProject::activate(
        "chart-inspector-dependencies",
        yss_project_model::ProjectData::new(),
    );
    let state = fixture.state();
    let project = state.capture_project_session().unwrap().instance_id;
    let mut catalog = state.read_project_index(&project).unwrap();
    let path = ChartResourcePath::parse("charts/Inspector.yssbi-chart").unwrap();
    catalog.charts.push(ProjectChartIndexEntry {
        chart_path: path.clone(),
        name: "Inspector".into(),
        database_id: "data".into(),
        chart_type: ChartType::Histogram,
        revision: ResourceRevision::INITIAL,
    });
    catalog.databases.push(ProjectDatabaseIndexEntry {
        id: "data".into(),
        resource_path: ProjectResourcePath::new("databases/data"),
        revision: ResourceRevision::INITIAL,
        engine: yss_database_contract::DatabaseEngine::Dataset {},
        schema_version: 1,
        required: true,
        name: Some("Original".into()),
    });
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let application = ApplicationState::new(Arc::new(ApplicationSessionSlot::new(
        NodeComponents::builtins().unwrap(),
    )));
    let tasks = Arc::new(ApplicationTasks::new(
        application,
        runtime.handle().clone(),
        DesktopEvents::default(),
    ));
    let (window, editor) = cx.update(|cx| {
        gpui_kit::init(cx);
        yss_ui::init(cx);
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| {
                ChartEditor::new(
                    tasks,
                    ChartRead {
                        project,
                        path,
                        revision: ResourceRevision::INITIAL,
                        catalog: Arc::new(catalog.clone()),
                        document: ChartDocument::new("data"),
                    },
                    window,
                    cx,
                )
            })
        })
        .unwrap()
    });
    let (before, captured_epoch) = cx.update(|cx| {
        (
            editor.read(cx).details_state().into_owned(),
            editor.read(cx).draft_epoch,
        )
    });
    catalog.publication_revision += 1;
    cx.update_window(window, |_, window, cx| {
        editor.update(cx, |editor, cx| {
            editor.replace_catalog(Arc::new(catalog.clone()), window, cx)
        });
        assert!(
            before == editor.read(cx).details_state(),
            "unrelated publications must not invalidate chart details"
        );
        editor.update(cx, |editor, cx| {
            editor.update_draft(
                captured_epoch,
                |draft| draft.chart_type = ChartType::Line,
                window,
                cx,
            )
        });
        assert_eq!(
            editor.read(cx).draft.chart_type,
            ChartType::Line,
            "retained menu callbacks must remain usable"
        );
        assert!(editor.read(cx).dirty());
    })
    .unwrap();

    let captured_epoch = cx.update(|cx| editor.read(cx).draft_epoch);
    catalog.publication_revision += 1;
    catalog.databases[0].name = Some("Renamed".into());
    cx.update_window(window, |_, window, cx| {
        editor.update(cx, |editor, cx| {
            editor.replace_catalog(Arc::new(catalog), window, cx)
        });
        assert_ne!(captured_epoch, editor.read(cx).draft_epoch);
        editor.update(cx, |editor, cx| {
            editor.update_draft(
                captured_epoch,
                |draft| draft.chart_type = ChartType::Scatter,
                window,
                cx,
            )
        });
        assert_eq!(
            editor.read(cx).draft.chart_type,
            ChartType::Line,
            "a menu opened against changed dataset choices must be stale"
        );
    })
    .unwrap();
}
