use crate::project::{
    NormalizedProjectRoot, ProjectFilesystemError, ProjectSession, ProjectState, ProjectStore,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use yss_graph_document::GraphResourcePath;
use yss_project_identity::ProjectInstanceId;
use yss_project_identity::ResourceRevision;
use yss_project_model::ProjectData;
use yss_variable_contract::VariableId;
use yss_variable_value::normalize_variable_tabular;
use yss_worksheet_document::WorksheetResourcePath;

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
    pub(crate) variable_revisions:
        HashMap<VariableId, crate::project::project_state::VariableRevisionEntry>,
    pub(crate) graph_revisions: HashMap<GraphResourcePath, yss_graph_document::GraphRevision>,
    pub(crate) worksheet_revisions: HashMap<WorksheetResourcePath, ResourceRevision>,
    pub(crate) authority_basis: Option<PreparedAuthorityBasis>,
    pub(crate) requires_final_rebuild: bool,
}

impl PreparedProjectActivation {
    pub(super) fn from_data(
        session_root: Option<NormalizedProjectRoot>,
        mut data: ProjectData,
        authority_basis: Option<PreparedAuthorityBasis>,
        requires_final_rebuild: bool,
    ) -> Result<Self, ProjectFilesystemError> {
        let store = ProjectStore::new();
        for variable in data.variables.values_mut() {
            let variable_id = variable.id;
            normalize_variable_tabular(variable).map_err(|error| {
                ProjectFilesystemError::TransactionPrepareFailed {
                    message: format!("variable '{variable_id}' is invalid: {error}"),
                }
            })?;
        }
        let graph_revisions = data
            .graphs
            .iter()
            .map(|(path, resource)| (path.clone(), resource.document.revision))
            .collect();
        let variable_revisions = data
            .variables
            .keys()
            .copied()
            .map(|id| {
                (
                    id,
                    crate::project::project_state::VariableRevisionEntry::present(
                        ResourceRevision::INITIAL,
                    ),
                )
            })
            .collect();
        let worksheet_revisions = data
            .worksheets
            .iter()
            .map(|(path, document)| (path.clone(), document.revision))
            .collect();
        Ok(Self {
            session_root,
            data,
            store,
            variable_revisions,
            graph_revisions,
            worksheet_revisions,
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
        self.run_activation_preparation_after_read_test_hook();
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

    #[cfg(test)]
    pub(crate) fn activate_project_fixture(&self, path: String, data: ProjectData) {
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
    use yss_variable_contract::{VariableInstance, VariableScope};

    #[test]
    fn activation_rejects_invalid_tabular_value_instead_of_silently_publishing_it() {
        let mut data = ProjectData::new();
        let variable_id = VariableId::new();
        data.variables.insert(
            variable_id,
            VariableInstance {
                id: variable_id,
                name: "invalid table".into(),
                data_type: DataType::DataFrame,
                data_value: DataValue::DataFrame("not-json".into()),
                tabular: None,
                description: String::new(),
                scope: VariableScope::Global,
                tags: Vec::new(),
            },
        );

        let result = PreparedProjectActivation::from_data(None, data, None, false);
        let Err(error) = result else {
            panic!("invalid tabular state must fail activation preparation");
        };
        assert_eq!(error.code(), "transaction_prepare_failed");
        assert!(error.to_string().contains(&variable_id.to_string()));
    }
}
