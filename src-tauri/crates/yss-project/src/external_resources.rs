use crate::{ProjectError, ProjectState};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs, io::Write, path::PathBuf};
use yss_project_filesystem::{metadata_is_redirect, read_secure_project_file};
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
        let _filesystem = self
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
            let metadata = read_secure_project_file(
                &root,
                &PathBuf::from(&receipt.resource_ref).join("resource.json"),
            )
            .map_err(|_| invalid())?;
            let existing: ExternalResourceReceipt =
                serde_json::from_slice(&metadata).map_err(ProjectError::Serialize)?;
            if existing != receipt {
                return Err(invalid());
            }
            for file in &receipt.files {
                let bytes = read_secure_project_file(
                    &root,
                    &PathBuf::from(&receipt.resource_ref).join(&file.name),
                )
                .map_err(|_| invalid())?;
                if yss_canonical_hash::content_sha256(&bytes) != file.sha256 {
                    return Err(invalid());
                }
            }
            return Ok(receipt);
        }
        // Save As already excludes this private transaction namespace.
        let stage = root
            .join(".yssbi-transaction")
            .join(format!("extension-{}", uuid::Uuid::new_v4()));
        let stage_parent = stage.parent().ok_or_else(invalid)?;
        fs::create_dir_all(stage_parent).map_err(ProjectError::Io)?;
        if metadata_is_redirect(&fs::symlink_metadata(stage_parent).map_err(ProjectError::Io)?) {
            return Err(invalid());
        }
        fs::create_dir(&stage).map_err(ProjectError::Io)?;
        let result = (|| {
            for artifact in artifacts {
                let mut file = fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(stage.join(artifact.name))
                    .map_err(ProjectError::Io)?;
                file.write_all(&artifact.contents)
                    .map_err(ProjectError::Io)?;
                file.sync_all().map_err(ProjectError::Io)?;
            }
            let mut metadata = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(stage.join("resource.json"))
                .map_err(ProjectError::Io)?;
            metadata
                .write_all(&serde_json::to_vec_pretty(&receipt).map_err(ProjectError::Serialize)?)
                .map_err(ProjectError::Io)?;
            metadata.sync_all().map_err(ProjectError::Io)?;
            drop(metadata);
            if self.project_instance_id() != instance.as_str()
                || self.project_session_id() != *session
            {
                return Err(invalid());
            }
            let parent = target.parent().ok_or_else(invalid)?;
            fs::create_dir_all(parent).map_err(ProjectError::Io)?;
            for path in [root.join("extension-results"), parent.to_path_buf()] {
                if metadata_is_redirect(&fs::symlink_metadata(path).map_err(ProjectError::Io)?) {
                    return Err(invalid());
                }
            }
            fs::rename(&stage, &target).map_err(ProjectError::Io)?;
            Ok(receipt)
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(stage);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn committed_plugin_results_are_idempotent_portable_and_current_project_bound() {
        let fixture = crate::fixtures::TempProject::activate(
            "external-result",
            yss_project_model::ProjectData::new(),
        );
        let state = fixture.state();
        let session = state.capture_project_session().unwrap();
        let project_session = state.project_session_id();
        let provenance = ExternalResourceProvenance {
            provider_id: "example.compute".into(),
            package_digest: "package-hash".into(),
            task_id: "task-1".into(),
            operation_id: "operation-1".into(),
            parameters_hash: "parameters-hash".into(),
            sources: vec![],
        };
        let artifacts = || {
            vec![ExternalArtifact {
                name: "summary.csv".into(),
                media_type: "text/csv".into(),
                contents: b"name,mean\na,2\n".to_vec(),
            }]
        };
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
        let tree =
            yss_project_filesystem::read_project_source_tree(session.root.as_path()).unwrap();
        assert!(
            tree.files
                .contains_key(&PathBuf::from(&receipt.resource_ref).join("summary.csv"))
        );
        assert!(
            tree.files
                .keys()
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
