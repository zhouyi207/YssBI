use crate::{ProjectSession, ProjectState, ProjectStore};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use yss_chart_document::ChartResourcePath;
use yss_graph_document::GraphResourcePath;
use yss_project_filesystem::{NormalizedProjectRoot, ProjectFilesystemError};
use yss_project_identity::ProjectInstanceId;
use yss_project_identity::ResourceRevision;
use yss_project_model::ProjectData;

#[derive(Clone, Default)]
pub(crate) struct ProjectActivationCoordinator {
    shared: Arc<ProjectActivationAdmission>,
}

#[derive(Default)]
struct ProjectActivationAdmission {
    owned: Mutex<bool>,
    available: Condvar,
}

pub(crate) struct ProjectActivationToken {
    shared: Arc<ProjectActivationAdmission>,
}

impl ProjectActivationCoordinator {
    pub(crate) fn acquire(&self) -> ProjectActivationToken {
        let mut owned = self
            .shared
            .owned
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while *owned {
            owned = self
                .shared
                .available
                .wait(owned)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        *owned = true;
        ProjectActivationToken {
            shared: Arc::clone(&self.shared),
        }
    }
}

impl Drop for ProjectActivationToken {
    fn drop(&mut self) {
        let mut owned = self
            .shared
            .owned
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *owned = false;
        drop(owned);
        self.shared.available.notify_one();
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedAuthorityBasis {
    pub project_instance_id: ProjectInstanceId,
    pub project_root: NormalizedProjectRoot,
    pub publication_revision: u64,
    pub authority_generation: u64,
}

pub struct PreparedProjectActivation {
    pub session_root: Option<NormalizedProjectRoot>,
    pub data: ProjectData,
    pub store: ProjectStore,
    pub(crate) graph_resource_revisions: HashMap<GraphResourcePath, ResourceRevision>,
    pub(crate) chart_revisions: HashMap<ChartResourcePath, ResourceRevision>,
    pub(crate) authority_basis: Option<PreparedAuthorityBasis>,
    pub(crate) requires_final_rebuild: bool,
}

impl PreparedProjectActivation {
    pub(super) fn from_data(
        session_root: Option<NormalizedProjectRoot>,
        data: ProjectData,
        authority_basis: Option<PreparedAuthorityBasis>,
        requires_final_rebuild: bool,
    ) -> Result<Self, ProjectFilesystemError> {
        for (path, graph) in &data.graphs {
            yss_graph_document::validate_constant_definitions(&graph.document.constants).map_err(
                |error| ProjectFilesystemError::TransactionPrepareFailed {
                    message: format!("graph '{path}' is invalid: {error}"),
                },
            )?;
        }
        let store = ProjectStore::new();
        let graph_resource_revisions = data
            .graphs
            .keys()
            .map(|path| (path.clone(), ResourceRevision::INITIAL))
            .collect();
        let chart_revisions = data
            .charts
            .keys()
            .map(|path| (path.clone(), ResourceRevision::INITIAL))
            .collect();
        Ok(Self {
            session_root,
            data,
            store,
            graph_resource_revisions,
            chart_revisions,
            authority_basis,
            requires_final_rebuild,
        })
    }
}

impl ProjectState {
    pub fn prepare_project_activation(
        &self,
        path: Option<&Path>,
    ) -> Result<PreparedProjectActivation, ProjectFilesystemError> {
        let Some(path) = path else {
            return PreparedProjectActivation::from_data(None, ProjectData::new(), None, false);
        };
        let root = NormalizedProjectRoot::from_project_path(path)?;
        let lease = self.filesystem().acquire(root.clone())?;
        let authority_before = self.capture_prepared_authority_basis(&root)?;
        let data = self.read_activation_data(&root)?;
        let authority_after = self.capture_prepared_authority_basis(&root)?;
        if authority_before != authority_after {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project authority changed during activation preparation".into(),
            });
        }
        let prepared =
            PreparedProjectActivation::from_data(Some(root), data, authority_after, true)?;
        drop(lease);
        Ok(prepared)
    }

    pub fn activate_prepared_project(
        &self,
        mut prepared: PreparedProjectActivation,
    ) -> Result<ProjectSession, ProjectFilesystemError> {
        let root =
            prepared
                .session_root
                .clone()
                .ok_or_else(|| ProjectFilesystemError::InvalidRoot {
                    path: PathBuf::new(),
                    message: "a pathless activation must use clear_project".into(),
                })?;
        let _activation = self.project_activation.acquire();
        self.run_project_activation_test_hook();
        let lease = self.filesystem().acquire(root.clone())?;
        if prepared.requires_final_rebuild {
            let authority_basis = prepared.authority_basis.take();
            let data = self.read_activation_data(&root)?;
            prepared = PreparedProjectActivation::from_data(
                Some(root.clone()),
                data,
                authority_basis,
                true,
            )?;
        }

        let published = self.publish_project_activation(prepared)?;
        drop(lease);
        let instance_id = published.dispose();
        // Activation publishes a complete declaration index rebuilt from the catalog. Old
        // session handoffs no longer require individual deltas; failed cleanup remains pending.
        let _ = yss_dataset_store::DatasetStore::open(root.as_path()).and_then(|store| {
            for publication in store.pending_publications()? {
                store.acknowledge_publication(&publication)?;
            }
            store.collect_garbage()?;
            Ok(())
        });
        Ok(ProjectSession { instance_id, root })
    }

    pub fn activate_project_from_path(
        &self,
        path: &Path,
    ) -> Result<ProjectSession, ProjectFilesystemError> {
        let prepared = self.prepare_project_activation(Some(path))?;
        self.activate_prepared_project(prepared)
    }

    pub fn clear_project(&self) -> Result<ProjectInstanceId, ProjectFilesystemError> {
        let prepared = self.prepare_project_activation(None)?;
        let _activation = self.project_activation.acquire();
        self.run_project_activation_test_hook();
        let published = self.publish_project_activation(prepared)?;
        Ok(published.dispose())
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn activate_project_fixture(&self, path: String, data: ProjectData) {
        let root = NormalizedProjectRoot::from_project_path(path).unwrap();
        self.activate_prepared_project(
            PreparedProjectActivation::from_data(Some(root), data, None, false).unwrap(),
        )
        .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_data_contract::{DataType, DataValue};

    #[test]
    fn activation_rejects_unsupported_schema_without_changing_files_or_session() {
        let mut data = ProjectData::new();
        data.graphs.insert(
            "events/Main.yssbi-event".parse().unwrap(),
            yss_project_model::GraphResourceDocument::new(
                "Main",
                yss_graph_document::GraphResourceKind::Event,
            ),
        );
        let fixture = crate::fixtures::TempProject::activate("unsupported-project-schema", data);
        let state = fixture.state();
        let session = state.capture_project_session().unwrap();
        let root = session.root.as_path();
        let manifest_path = root.join(yss_project_layout::PROJECT_METADATA_FILE);
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
        manifest["schemaVersion"] = serde_json::json!(3);
        let manifest = serde_json::to_vec(&manifest).unwrap();
        std::fs::write(&manifest_path, &manifest).unwrap();
        let variables_path = root.join("variables.yssbi-vars");
        let variables = br#"{"variables":{}}"#;
        std::fs::write(&variables_path, variables).unwrap();
        let graph_path = root.join("events/Main.yssbi-event");
        let graph = std::fs::read(&graph_path).unwrap();

        let error = state.activate_project_from_path(root).unwrap_err();

        assert_eq!(error.code(), "transaction_prepare_failed");
        assert_eq!(std::fs::read(&manifest_path).unwrap(), manifest);
        assert_eq!(std::fs::read(&variables_path).unwrap(), variables);
        assert_eq!(std::fs::read(&graph_path).unwrap(), graph);
        assert_eq!(
            state.capture_project_session().unwrap().instance_id,
            session.instance_id
        );
    }

    #[test]
    fn activation_rejects_invalid_tabular_value_instead_of_silently_publishing_it() {
        let mut data = ProjectData::new();
        let id = yss_graph_document::ConstantId::new();
        let mut graph = yss_project_model::GraphResourceDocument::new(
            "Main",
            yss_graph_document::GraphResourceKind::Event,
        );
        graph.document.constants.insert(
            id,
            yss_graph_document::GraphConstant {
                id,
                name: "invalid table".into(),
                data_type: DataType::DataFrame,
                data_value: DataValue::DataFrame("not-json".into()),
                tabular: None,
                description: String::new(),
                tags: vec![],
            },
        );
        data.graphs
            .insert("events/Main.yssbi-event".parse().unwrap(), graph);

        let result = PreparedProjectActivation::from_data(None, data, None, false);
        let Err(error) = result else {
            panic!("invalid tabular state must fail activation preparation");
        };
        assert_eq!(error.code(), "transaction_prepare_failed");
        assert!(error.to_string().contains(&id.to_string()));
    }
}
