use crate::file_resources::{self, ResourceFile};
use std::collections::HashMap;
use yss_chart_document::ChartDocument;
use yss_project_history::{ChartResourceKey, ResourceKey, ResourceLifecycleKind};
use yss_project_model::{
    ProjectData, ProjectDataPatch,
    chart::{ChartPath, ChartState},
    file::FilePatch,
};
pub type ChartSnapshot = file_resources::FileSnapshot<ChartDocument>;
pub type ChartIndexEntry = file_resources::FileIndexEntry<ChartDocument>;
pub type ChartCommand = file_resources::FileCommand<ChartDocument>;
pub type ChartCommandResult = file_resources::FileCommandResult<ChartDocument>;
impl ResourceFile for ChartDocument {
    const LIFECYCLE_KIND: ResourceLifecycleKind = ResourceLifecycleKind::Chart;
    fn files(data: &ProjectData) -> &HashMap<ChartPath, ChartState> {
        &data.charts
    }
    fn files_mut(data: &mut ProjectData) -> &mut HashMap<ChartPath, ChartState> {
        &mut data.charts
    }
    fn patch(patch: FilePatch<Self>) -> ProjectDataPatch {
        ProjectDataPatch::Chart(patch)
    }
    fn key(path: &ChartPath) -> ResourceKey {
        ResourceKey::Chart(ChartResourceKey(path.as_str().into()))
    }
    // A new chart binds the first dataset so the editor can preview without another step.
    fn initial(_name: &str, data: &ProjectData) -> Self {
        ChartDocument::new(data.databases.keys().min().cloned().unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProjectOperationError, ProjectState, file_resources::FileCommand};
    use yss_chart_document::ChartType;
    use yss_project_identity::{OperationId, ProjectInstanceId};
    use yss_project_model::{chart::ChartEdit, file::FileContent};

    fn apply(
        state: &ProjectState,
        project: &ProjectInstanceId,
        command: ChartCommand,
    ) -> ChartSnapshot {
        state
            .apply_file_command(project, OperationId::new(), command)
            .unwrap()
            .snapshot
            .unwrap()
    }

    fn create(state: &ProjectState, project: &ProjectInstanceId, name: &str) -> ChartSnapshot {
        apply(state, project, FileCommand::Create { name: name.into() })
    }

    fn save(
        state: &ProjectState,
        project: &ProjectInstanceId,
        chart: ChartSnapshot,
        document: ChartDocument,
    ) -> ChartSnapshot {
        let edited = apply(
            state,
            project,
            FileCommand::Edit {
                path: chart.path,
                version: chart.version,
                edits: vec![ChartEdit::SetDocument { document }],
            },
        );
        apply(
            state,
            project,
            FileCommand::Save {
                path: edited.path,
                version: edited.version,
            },
        )
    }

    #[test]
    fn new_charts_bind_the_first_dataset() {
        let mut data = ProjectData::new();
        assert_eq!(ChartDocument::initial("Report", &data).database_id, "");
        for id in ["sales", "inventory"] {
            data.databases.insert(
                id.into(),
                yss_database_contract::DatabaseDecl {
                    id: yss_database_contract::DatabaseId::from_existing(id.into()),
                    engine: yss_database_contract::DatabaseEngine::Dataset {},
                    schema_version: 1,
                    required: false,
                    name: id.into(),
                },
            );
        }
        assert_eq!(
            ChartDocument::initial("Report", &data).database_id,
            "inventory"
        );
    }

    #[test]
    fn created_chart_saves_the_strict_format_and_advances_its_index_revision() {
        let fixture = crate::fixtures::TempProject::activate("chart-create", ProjectData::new());
        let state = fixture.state();
        let session = state.capture_project_session().unwrap();
        let project = &session.instance_id;
        let created = create(state, project, "Report");
        assert!(!created.dirty);

        let mut document = created.content.clone();
        document.chart_type = ChartType::Line;
        document.encodings.x = Some("month".into());
        let saved = save(state, project, created, document.clone());
        assert!(!saved.dirty);
        let disk = std::fs::read(session.root.as_path().join(saved.path.as_str())).unwrap();
        assert_eq!(ChartDocument::decode(&disk).unwrap(), document);
        let index = state.read_project_index(project).unwrap();
        assert_eq!(index.charts.len(), 1);
        assert_eq!(index.charts[0].path, saved.path);
        assert_eq!(index.charts[0].revision, saved.version.revision);
    }

    #[test]
    fn renaming_into_a_deleted_path_does_not_authorize_its_old_draft() {
        let fixture = crate::fixtures::TempProject::activate("chart-rename", ProjectData::new());
        let state = fixture.state();
        let session = state.capture_project_session().unwrap();
        let project = &session.instance_id;
        let previous = create(state, project, "Previous");
        let mut old_draft = previous.content.clone();
        old_draft.chart_type = ChartType::Scatter;
        let previous = save(state, project, previous, old_draft.clone());
        state
            .apply_file_command(
                project,
                OperationId::new(),
                FileCommand::Delete {
                    path: previous.path.clone(),
                    version: previous.version.clone(),
                },
            )
            .unwrap();
        let current = create(state, project, "Current");
        let moved = apply(
            state,
            project,
            FileCommand::Rename {
                path: current.path.clone(),
                version: current.version,
                name: "Previous".into(),
            },
        );
        assert_eq!(moved.path, previous.path);

        let stale = state.apply_file_command(
            project,
            OperationId::new(),
            FileCommand::Edit {
                path: previous.path.clone(),
                version: previous.version,
                edits: vec![ChartEdit::SetDocument {
                    document: old_draft,
                }],
            },
        );
        assert!(matches!(
            stale,
            Err(ProjectOperationError::ResourceRevisionConflict { .. })
        ));
        assert!(matches!(
            state.read_file(project, &current.path),
            Err(ProjectOperationError::ResourceNotFound { .. })
        ));
        let disk = std::fs::read(session.root.as_path().join(moved.path.as_str())).unwrap();
        assert_eq!(ChartDocument::decode(&disk).unwrap(), moved.content);
    }

    #[test]
    fn chart_directory_reads_reject_unsupported_documents_and_ambiguous_names() {
        use crate::file_resources::{file_index, load_files, scan_file_paths};
        let root = std::env::temp_dir().join(format!("yssbi-chart-scan-{}", uuid::Uuid::new_v4()));
        let charts = root.join(yss_project_layout::CHARTS_DIR);
        std::fs::create_dir_all(&charts).unwrap();
        let mut value = serde_json::to_value(ChartDocument::new("db")).unwrap();
        value["schemaVersion"] =
            serde_json::json!(yss_chart_document::CURRENT_CHART_SCHEMA_VERSION + 1);
        std::fs::write(
            charts.join("Unsupported.yssbi-chart"),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
        assert!(load_files::<ChartDocument>(&root).is_err());
        std::fs::remove_file(charts.join("Unsupported.yssbi-chart")).unwrap();

        // Unrelated files follow the shared authored-directory rule and are not resources.
        std::fs::write(charts.join(".DS_Store"), b"finder").unwrap();
        std::fs::write(charts.join("Notes.json"), b"{}").unwrap();
        assert!(file_index::<ChartDocument>(&root).unwrap().is_empty());

        for name in ["Straße", "STRASSE"] {
            std::fs::write(
                charts.join(format!("{name}.yssbi-chart")),
                ChartDocument::new("db").encode().unwrap(),
            )
            .unwrap();
        }
        assert!(scan_file_paths::<ChartDocument>(&root).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn chart_reads_reject_symlinked_files_before_reading_them() {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!("yssbi-chart-link-{}", uuid::Uuid::new_v4()));
        let external =
            std::env::temp_dir().join(format!("yssbi-chart-outside-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join(yss_project_layout::CHARTS_DIR)).unwrap();
        std::fs::create_dir_all(&external).unwrap();
        let target = external.join("External.yssbi-chart");
        std::fs::write(&target, ChartDocument::new("db").encode().unwrap()).unwrap();
        symlink(
            &target,
            root.join(yss_project_layout::CHARTS_DIR)
                .join("External.yssbi-chart"),
        )
        .unwrap();

        assert!(crate::file_resources::load_files::<ChartDocument>(&root).is_err());
        assert!(crate::file_resources::file_index::<ChartDocument>(&root).is_err());
        assert!(target.is_file());
        std::fs::remove_dir_all(root).unwrap();
        std::fs::remove_dir_all(external).unwrap();
    }
}
