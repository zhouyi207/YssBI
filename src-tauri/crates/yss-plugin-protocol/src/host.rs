use crate::{PluginFailure, PluginManifest};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectContext {
    pub project_instance_id: String,
    pub project_session_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CallContext {
    pub context_id: String,
    pub plugin_id: String,
    pub installation_generation: String,
    pub instance_id: String,
    pub package_digest: String,
    pub project: Option<ProjectContext>,
    pub task_id: Option<String>,
    pub operation_id: Option<String>,
    pub parameters_hash: Option<String>,
}

pub trait HostServices: Send + Sync {
    fn current_project(&self) -> Result<Option<ProjectContext>, PluginFailure>;
    fn invoke(
        &self,
        context: &CallContext,
        method: &str,
        input: Value,
        exchange_dir: &Path,
    ) -> Result<Value, PluginFailure>;
    fn release_context(&self, _context_id: &str) {}
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstalledPlugin {
    pub manifest: PluginManifest,
    pub package_digest: String,
    pub installation_generation: String,
    pub enabled: bool,
    pub process_state: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageInspection {
    pub manifest: PluginManifest,
    pub package_digest: String,
    pub signer_key: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ViewSession {
    pub session_id: String,
    pub html: String,
    pub installation_generation: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskSnapshot {
    pub task_id: String,
    pub operation_id: String,
    pub plugin_id: String,
    pub package_digest: String,
    pub state: TaskState,
    pub revision: String,
    pub error: Option<PluginFailure>,
    pub result: Option<Value>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum TaskState {
    Admitted,
    Running,
    CancelRequested,
    Succeeded,
    Failed,
    Cancelled,
    OutcomeUnknown,
}
impl TaskState {
    pub fn terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::OutcomeUnknown
        )
    }
}
