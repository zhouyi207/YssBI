use crate::execution::ApplicationState;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::Mutex,
};
use yss_database_contract::DatabaseId;
use yss_database_runtime::session_api::{DatabaseColumnSelection, DatabaseDataSnapshotRequest};
use yss_plugin_protocol::{
    CallContext, HostServices, PluginFailure, ProjectContext, valid_relative_path,
};
use yss_project::external_resources::{ExternalResourceProvenance, ExternalSourceSnapshot};
use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

struct Lease {
    context: String,
    path: PathBuf,
}
pub struct PluginHostServices {
    application: ApplicationState,
    leases: Mutex<BTreeMap<String, Lease>>,
    sources: Mutex<BTreeMap<String, Vec<ExternalSourceSnapshot>>>,
}
impl PluginHostServices {
    pub fn new(application: ApplicationState) -> Self {
        Self {
            application,
            leases: Mutex::new(BTreeMap::new()),
            sources: Mutex::new(BTreeMap::new()),
        }
    }
}
fn fail(code: &str) -> PluginFailure {
    PluginFailure::new(code)
}

impl HostServices for PluginHostServices {
    fn release_context(&self, context_id: &str) {
        if let Ok(mut sources) = self.sources.lock() {
            sources.remove(context_id);
        }
        let paths = {
            let Ok(mut leases) = self.leases.lock() else {
                return;
            };
            let ids = leases
                .iter()
                .filter(|(_, lease)| lease.context == context_id)
                .map(|(id, _)| id.clone())
                .collect::<Vec<_>>();
            ids.into_iter()
                .filter_map(|id| leases.remove(&id).map(|lease| lease.path))
                .collect::<Vec<_>>()
        };
        for path in paths {
            let _ = fs::remove_file(path);
        }
    }
    fn current_project(&self) -> Result<Option<ProjectContext>, PluginFailure> {
        let Ok(session) = self.application.capture_session() else {
            return Ok(None);
        };
        if session.project().get_path().is_none() {
            return Ok(None);
        }
        Ok(Some(ProjectContext {
            project_instance_id: session.project_instance_id().as_str().into(),
            project_session_id: session.project_session_id().as_str().into(),
        }))
    }
    fn invoke(
        &self,
        context: &CallContext,
        method: &str,
        input: Value,
        exchange_dir: &Path,
    ) -> Result<Value, PluginFailure> {
        if context.project != self.current_project()? {
            return Err(fail("plugin_stale_context"));
        }
        let project = context
            .project
            .as_ref()
            .ok_or_else(|| fail("plugin_project_required"))?;
        let captured = self
            .application
            .capture_session()
            .map_err(|_| fail("plugin_stale_context"))?;
        match method {
            "data.list" => {
                let data = self
                    .application
                    .query_project_databases()
                    .map_err(|_| fail("plugin_dataset_unavailable"))?;
                Ok(
                    json!({"datasets":data.databases().iter().map(|dataset|json!({"id":dataset.declaration.id.as_str(),"name":dataset.declaration.name,"columns":dataset.schema.columns().iter().map(|column|json!({"name":column.name().as_str(),"type":column.display_type(),"nullable":column.nullable()})).collect::<Vec<_>>()})).collect::<Vec<_>>()}),
                )
            }
            "data.snapshot" => {
                let dataset = input["datasetId"]
                    .as_str()
                    .ok_or_else(|| fail("plugin_dataset_invalid"))?;
                let columns = input["columns"]
                    .as_array()
                    .filter(|columns| !columns.is_empty() && columns.len() <= 64)
                    .ok_or_else(|| fail("plugin_dataset_invalid"))?;
                let columns = columns
                    .iter()
                    .map(|value| {
                        yss_tabular_contract::TabularColumnName::try_from(
                            value.as_str().unwrap_or_default(),
                        )
                        .map_err(|_| fail("plugin_dataset_invalid"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if !captured
                    .project()
                    .get_data()
                    .map_err(|_| fail("plugin_stale_context"))?
                    .databases
                    .contains_key(dataset)
                {
                    return Err(fail("plugin_dataset_invalid"));
                }
                let max_rows = (128 * 1024 * 1024 / (columns.len() * 64)).min(1_000_000);
                let selected_columns = columns
                    .iter()
                    .map(|column| column.as_str().to_owned())
                    .collect();
                let snapshot = yss_database_runtime::session_api::arrow_snapshot(
                    captured.database(),
                    DatabaseDataSnapshotRequest {
                        database: DatabaseId::from_existing(dataset.into()),
                        columns: DatabaseColumnSelection::Selected(columns.into_boxed_slice()),
                        offset: 0,
                        limit: max_rows + 1,
                    },
                )
                .map_err(|_| fail("plugin_dataset_unavailable"))?;
                if snapshot.row_count > max_rows {
                    return Err(fail("plugin_resource_exhausted"));
                }
                let directory = exchange_dir.join("snapshots");
                fs::create_dir_all(&directory).map_err(|_| fail("plugin_storage_failed"))?;
                if yss_project_filesystem::metadata_is_redirect(
                    &fs::symlink_metadata(&directory).map_err(|_| fail("plugin_storage_failed"))?,
                ) {
                    return Err(fail("plugin_permission_denied"));
                }
                let id = uuid::Uuid::new_v4().to_string();
                let path = directory.join(format!("{id}.arrow"));
                yss_tabular_io::write_ipc_batches(
                    &path,
                    &snapshot.schema,
                    snapshot.batches.iter().cloned().map(Ok),
                )
                .map_err(|_| fail("plugin_snapshot_failed"))?;
                let bytes = fs::metadata(&path)
                    .map_err(|_| fail("plugin_snapshot_failed"))?
                    .len();
                if bytes > 128 * 1024 * 1024
                    || self
                        .application
                        .revalidate_captured_session(&captured)
                        .is_err()
                {
                    let _ = fs::remove_file(path);
                    return Err(fail("plugin_stale_context"));
                }
                let snapshot_bytes = bounded_bytes(&path, 128 * 1024 * 1024)?;
                let snapshot_hash = yss_canonical_hash::content_sha256(&snapshot_bytes);
                let mut leases = self
                    .leases
                    .lock()
                    .map_err(|_| fail("plugin_state_unavailable"))?;
                if leases.len() >= 32 {
                    let _ = fs::remove_file(path);
                    return Err(fail("plugin_resource_exhausted"));
                }
                let mut sources = self
                    .sources
                    .lock()
                    .map_err(|_| fail("plugin_state_unavailable"))?;
                let source = sources.entry(context.context_id.clone()).or_default();
                if source.len() >= 32 {
                    let _ = fs::remove_file(path);
                    return Err(fail("plugin_resource_exhausted"));
                }
                source.push(ExternalSourceSnapshot {
                    dataset_id: dataset.into(),
                    runtime_revision: snapshot.runtime_revision().get().to_string(),
                    content_sha256: snapshot_hash.clone(),
                    columns: selected_columns,
                });
                leases.insert(
                    id.clone(),
                    Lease {
                        context: context.context_id.clone(),
                        path: path.clone(),
                    },
                );
                Ok(
                    json!({"leaseId":id,"path":path,"sourceRevision":snapshot.runtime_revision().get().to_string(),"sha256":snapshot_hash,"rows":snapshot.row_count.to_string(),"bytes":bytes.to_string(),"format":"arrowIpc"}),
                )
            }
            "data.release" => {
                let id = input["leaseId"]
                    .as_str()
                    .ok_or_else(|| fail("plugin_lease_invalid"))?;
                let lease = {
                    let mut leases = self
                        .leases
                        .lock()
                        .map_err(|_| fail("plugin_state_unavailable"))?;
                    if !leases
                        .get(id)
                        .is_some_and(|lease| lease.context == context.context_id)
                    {
                        return Err(fail("plugin_permission_denied"));
                    }
                    leases.remove(id)
                };
                if let Some(lease) = lease {
                    let _ = fs::remove_file(lease.path);
                }
                Ok(Value::Null)
            }
            "results.commit" => {
                let task_id = context
                    .task_id
                    .as_deref()
                    .ok_or_else(|| fail("plugin_task_required"))?;
                let files = input["artifacts"]
                    .as_array()
                    .filter(|files| !files.is_empty() && files.len() <= 16)
                    .ok_or_else(|| fail("plugin_artifact_invalid"))?;
                let root =
                    fs::canonicalize(exchange_dir).map_err(|_| fail("plugin_artifact_invalid"))?;
                let mut artifacts = Vec::new();
                let mut bytes = 0u64;
                for entry in files {
                    let relative = entry["path"]
                        .as_str()
                        .filter(|path| valid_relative_path(path))
                        .ok_or_else(|| fail("plugin_artifact_invalid"))?;
                    let path = fs::canonicalize(root.join(relative))
                        .map_err(|_| fail("plugin_artifact_invalid"))?;
                    if !path.starts_with(&root) {
                        return Err(fail("plugin_artifact_invalid"));
                    }
                    let mut component = root.clone();
                    for part in relative.split('/') {
                        component.push(part);
                        if yss_project_filesystem::metadata_is_redirect(
                            &fs::symlink_metadata(&component)
                                .map_err(|_| fail("plugin_artifact_invalid"))?,
                        ) {
                            return Err(fail("plugin_artifact_invalid"));
                        }
                    }
                    let contents = bounded_bytes(&path, 128 * 1024 * 1024 - bytes)?;
                    bytes += contents.len() as u64;
                    artifacts.push(yss_project::external_resources::ExternalArtifact {
                        name: path
                            .file_name()
                            .and_then(|value| value.to_str())
                            .ok_or_else(|| fail("plugin_artifact_invalid"))?
                            .into(),
                        media_type: entry["mediaType"]
                            .as_str()
                            .unwrap_or("application/octet-stream")
                            .into(),
                        contents,
                    });
                }
                self.application
                    .revalidate_captured_session(&captured)
                    .map_err(|_| fail("plugin_stale_context"))?;
                let sources = self
                    .sources
                    .lock()
                    .map_err(|_| fail("plugin_state_unavailable"))?
                    .get(&context.context_id)
                    .cloned()
                    .unwrap_or_default();
                let receipt = captured
                    .project()
                    .commit_external_artifacts(
                        &ProjectInstanceId::from_existing(
                            project.project_instance_id.clone().into(),
                        ),
                        &ProjectSessionId::new(project.project_session_id.clone()),
                        ExternalResourceProvenance {
                            provider_id: context.plugin_id.clone(),
                            package_digest: context.package_digest.clone(),
                            task_id: task_id.into(),
                            operation_id: context
                                .operation_id
                                .clone()
                                .ok_or_else(|| fail("plugin_task_required"))?,
                            parameters_hash: context
                                .parameters_hash
                                .clone()
                                .ok_or_else(|| fail("plugin_task_required"))?,
                            sources,
                        },
                        artifacts,
                    )
                    .map_err(|_| fail("plugin_result_commit_failed"))?;
                serde_json::to_value(receipt).map_err(|_| fail("plugin_result_commit_failed"))
            }
            _ => Err(fail("plugin_method_unknown")),
        }
    }
}

fn bounded_bytes(path: &Path, limit: u64) -> Result<Vec<u8>, PluginFailure> {
    let file = fs::File::open(path).map_err(|_| fail("plugin_artifact_invalid"))?;
    let metadata = file
        .metadata()
        .map_err(|_| fail("plugin_artifact_invalid"))?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(fail("plugin_resource_exhausted"));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| fail("plugin_artifact_invalid"))?;
    if bytes.len() as u64 > limit {
        return Err(fail("plugin_resource_exhausted"));
    }
    Ok(bytes)
}
