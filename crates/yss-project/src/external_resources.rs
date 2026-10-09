use crate::{ProjectError, ProjectState};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs, path::PathBuf};
use yss_filesystem::{
    FilesystemError, FilesystemTransaction, StagedFilesystemMutation, TransactionContext,
    TransactionId, read_secure_file,
};
use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

pub struct ExternalArtifact {
    pub name: String,
    pub media_type: String,
    pub contents: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSourceSnapshot {
    pub dataset_id: String,
    pub runtime_revision: String,
    pub content_sha256: String,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalResourceProvenance {
    pub provider_id: String,
    pub package_digest: String,
    pub task_id: String,
    pub operation_id: String,
    pub parameters_hash: String,
    pub sources: Vec<ExternalSourceSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalResourceFile {
    pub name: String,
    pub media_type: String,
    pub size: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalResourceReceipt {
    pub schema_version: u32,
    pub resource_ref: String,
    pub provenance: ExternalResourceProvenance,
    pub files: Vec<ExternalResourceFile>,
}

fn valid_name(value: &str) -> bool {
    let stem = value
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    !value.is_empty()
        && value.len() <= 96
        && !value.ends_with('.')
        && !value.contains("..")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        && !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        && !(stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
}

fn transaction_error(error: FilesystemError) -> ProjectError {
    ProjectError::InvalidProjectFormat(error.to_string())
}

impl ProjectState {
    pub fn commit_external_artifacts(
        &self,
        instance: &ProjectInstanceId,
        session: &ProjectSessionId,
        provenance: ExternalResourceProvenance,
        mut artifacts: Vec<ExternalArtifact>,
    ) -> Result<ExternalResourceReceipt, ProjectError> {
        let invalid =
            || ProjectError::InvalidProjectFormat("invalid external artifact transaction".into());
        if !valid_name(&provenance.provider_id)
            || !valid_name(&provenance.task_id)
            || !valid_name(&provenance.operation_id)
            || artifacts.is_empty()
            || artifacts.len() > 16
        {
            return Err(invalid());
        }
        let captured = self.capture_project_session().map_err(|_| invalid())?;
        if captured.instance_id != *instance {
            return Err(invalid());
        }
        let filesystem = self
            .acquire_filesystem_lease(captured.root.clone())
            .map_err(|_| invalid())?;
        self.validate_project_session(&captured)
            .map_err(|_| invalid())?;
        if self.project_session_id() != *session {
            return Err(invalid());
        }
        let root = captured.root.as_path().to_path_buf();
        let relative = format!(
            "extension-results/{}/{}",
            provenance.provider_id, provenance.task_id
        );
        let target = root.join(&relative);
        artifacts.sort_by(|a, b| a.name.cmp(&b.name));
        let mut names = BTreeSet::new();
        let mut total = 0usize;
        for artifact in &artifacts {
            if !valid_name(&artifact.name)
                || artifact.name.eq_ignore_ascii_case("resource.json")
                || !names.insert(artifact.name.to_ascii_lowercase())
            {
                return Err(invalid());
            }
            total = total
                .checked_add(artifact.contents.len())
                .ok_or_else(invalid)?;
            if total > 128 * 1024 * 1024
                || artifact.media_type.len() > 128
                || artifact.media_type.contains(['\r', '\n'])
            {
                return Err(invalid());
            }
        }
        let receipt = ExternalResourceReceipt {
            schema_version: 1,
            resource_ref: relative,
            provenance,
            files: artifacts
                .iter()
                .map(|artifact| ExternalResourceFile {
                    name: artifact.name.clone(),
                    media_type: artifact.media_type.clone(),
                    size: artifact.contents.len().to_string(),
                    sha256: yss_canonical_hash::content_sha256(&artifact.contents),
                })
                .collect(),
        };
        if target.exists() {
            let metadata = read_secure_file(
                &root,
                &PathBuf::from(&receipt.resource_ref).join("resource.json"),
            )
            .map_err(|_| invalid())?;
            let existing: ExternalResourceReceipt =
                serde_json::from_slice(&metadata).map_err(ProjectError::Deserialize)?;
            if existing != receipt {
                return Err(invalid());
            }
            for file in &receipt.files {
                let bytes = read_secure_file(
                    &root,
                    &PathBuf::from(&receipt.resource_ref).join(&file.name),
                )
                .map_err(|_| invalid())?;
                if yss_canonical_hash::content_sha256(&bytes) != file.sha256 {
                    return Err(invalid());
                }
            }
            self.validate_project_session(&captured)
                .map_err(|_| invalid())?;
            if self.project_session_id() != *session {
                return Err(invalid());
            }
            return Ok(receipt);
        }
        let relative = PathBuf::from(&receipt.resource_ref);
        let mut mutations = Vec::with_capacity(artifacts.len() + 1);
        mutations.extend(
            artifacts
                .into_iter()
                .map(|artifact| StagedFilesystemMutation::Write {
                    relative_path: relative.join(artifact.name),
                    contents: artifact.contents,
                }),
        );
        mutations.push(StagedFilesystemMutation::Write {
            relative_path: relative.join("resource.json"),
            contents: serde_json::to_vec_pretty(&receipt).map_err(ProjectError::Serialize)?,
        });
        let prepared = FilesystemTransaction::prepare(
            TransactionContext {
                root: captured.root.clone(),
                transaction_id: TransactionId::new(),
                recovery_marker: Some(self.project_recovery_marker()),
            },
            filesystem,
            mutations,
        )
        .map_err(transaction_error)?;
        self.validate_project_session(&captured)
            .map_err(|_| invalid())?;
        if self.project_session_id() != *session {
            return Err(invalid());
        }
        match fs::symlink_metadata(&target) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(ProjectError::Io(error)),
            Ok(_) => return Err(invalid()),
        }
        let committed = prepared.commit().map_err(transaction_error)?;
        if self.validate_project_session(&captured).is_err()
            || self.project_session_id() != *session
        {
            committed.rollback().map_err(transaction_error)?;
            return Err(invalid());
        }
        committed.finalize();
        Ok(receipt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provenance() -> ExternalResourceProvenance {
        ExternalResourceProvenance {
            provider_id: "example.compute".into(),
            package_digest: "package-hash".into(),
            task_id: "task-1".into(),
            operation_id: "operation-1".into(),
            parameters_hash: "parameters-hash".into(),
            sources: vec![],
        }
    }

    fn artifacts() -> Vec<ExternalArtifact> {
        vec![ExternalArtifact {
            name: "summary.csv".into(),
            media_type: "text/csv".into(),
            contents: b"name,mean\na,2\n".to_vec(),
        }]
    }

    #[cfg(unix)]
    #[test]
    fn external_artifacts_reject_redirected_parents_without_creating_outside_directories() {
        let fixture = crate::fixtures::TempProject::activate(
            "external-redirect",
            yss_project_model::ProjectData::new(),
        );
        let outside = crate::fixtures::TempProject::activate(
            "external-redirect-outside",
            yss_project_model::ProjectData::new(),
        );
        let state = fixture.state();
        let session = state.capture_project_session().unwrap();
        let outside_session = outside.state().capture_project_session().unwrap();
        std::os::unix::fs::symlink(
            outside_session.root.as_path(),
            session.root.as_path().join("extension-results"),
        )
        .unwrap();
        let result = state.commit_external_artifacts(
            &session.instance_id,
            &state.project_session_id(),
            provenance(),
            artifacts(),
        );
        assert!(result.is_err());
        assert!(
            !outside_session
                .root
                .as_path()
                .join("example.compute")
                .exists(),
            "a rejected result commit created a directory outside the project"
        );
    }

    #[test]
    fn external_artifact_failure_rolls_back_the_entire_result_directory() {
        let fixture = crate::fixtures::TempProject::activate(
            "external-rollback",
            yss_project_model::ProjectData::new(),
        );
        let state = fixture.state();
        let session = state.capture_project_session().unwrap();
        let provenance = provenance();
        state.set_filesystem_fault(Some(
            yss_filesystem::FilesystemFaultPoint::SecondLiveReplacement,
        ));
        assert!(
            state
                .commit_external_artifacts(
                    &session.instance_id,
                    &state.project_session_id(),
                    provenance.clone(),
                    artifacts(),
                )
                .is_err()
        );
        assert!(
            !session
                .root
                .as_path()
                .join("extension-results/example.compute/task-1")
                .exists()
        );
        state.ensure_project_operational().unwrap();
        let receipt = state
            .commit_external_artifacts(
                &session.instance_id,
                &state.project_session_id(),
                provenance,
                artifacts(),
            )
            .unwrap();
        assert_eq!(
            read_secure_file(
                session.root.as_path(),
                &PathBuf::from(receipt.resource_ref).join("summary.csv")
            )
            .unwrap(),
            b"name,mean\na,2\n"
        );
    }

    #[test]
    fn committed_plugin_results_are_idempotent_portable_and_current_project_bound() {
        let fixture = crate::fixtures::TempProject::activate(
            "external-result",
            yss_project_model::ProjectData::new(),
        );
        let state = fixture.state();
        let session = state.capture_project_session().unwrap();
        let project_session = state.project_session_id();
        let provenance = provenance();
        let receipt = state
            .commit_external_artifacts(
                &session.instance_id,
                &project_session,
                provenance.clone(),
                artifacts(),
            )
            .unwrap();
        assert_eq!(
            state
                .commit_external_artifacts(
                    &session.instance_id,
                    &project_session,
                    provenance.clone(),
                    artifacts()
                )
                .unwrap(),
            receipt
        );
        let mut changed = artifacts();
        changed[0].contents.push(b'3');
        assert!(
            state
                .commit_external_artifacts(
                    &session.instance_id,
                    &project_session,
                    provenance.clone(),
                    changed
                )
                .is_err()
        );
        let tree = yss_filesystem::read_file_inventory(session.root.as_path()).unwrap();
        assert!(
            tree.files
                .contains(&PathBuf::from(&receipt.resource_ref).join("summary.csv"))
        );
        assert!(
            tree.files
                .iter()
                .all(|path| !path.starts_with(".yssbi-transaction"))
        );
        assert!(
            state
                .commit_external_artifacts(
                    &ProjectInstanceId::new(),
                    &project_session,
                    provenance.clone(),
                    artifacts()
                )
                .is_err()
        );
        assert!(
            state
                .commit_external_artifacts(
                    &session.instance_id,
                    &ProjectSessionId::new("another-session"),
                    provenance.clone(),
                    artifacts()
                )
                .is_err()
        );
        let metadata = session
            .root
            .as_path()
            .join(&receipt.resource_ref)
            .join("resource.json");
        std::fs::write(metadata, b"damaged receipt").unwrap();
        assert!(matches!(
            state.commit_external_artifacts(
                &session.instance_id,
                &project_session,
                provenance.clone(),
                artifacts(),
            ),
            Err(ProjectError::Deserialize(_))
        ));
        let _lifecycle = state
            .filesystem_for_test()
            .begin_root_lifecycle(session.root.clone())
            .unwrap();
        assert!(
            state
                .commit_external_artifacts(
                    &session.instance_id,
                    &project_session,
                    provenance,
                    artifacts()
                )
                .is_err()
        );
    }
}
