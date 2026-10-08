use crate::{PluginFailure, PluginManifest, valid_id};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

pub const MAX_NATIVE_VIEW_BYTES: u64 = 256 * 1024;

/// A signed native form. The host owns rendering and transient edits.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeView {
    pub description: String,
    pub fields: Vec<NativeField>,
    pub actions: Vec<NativeAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeField {
    pub id: String,
    pub label: String,
    pub input: NativeInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum NativeInput {
    Text { value: String, multiline: bool },
    Number { value: f64 },
    Boolean { value: bool },
    Choice { value: String, options: Vec<String> },
    Json { value: Value },
}

impl NativeInput {
    pub fn value(&self) -> Value {
        match self {
            Self::Text { value, .. } | Self::Choice { value, .. } => Value::String(value.clone()),
            Self::Number { value } => Value::from(*value),
            Self::Boolean { value } => Value::Bool(*value),
            Self::Json { value } => value.clone(),
        }
    }

    pub fn accepts(&self, value: &Value) -> bool {
        match self {
            Self::Text { .. } => value.as_str().is_some_and(|text| text.len() <= 64 * 1024),
            Self::Number { .. } => value.as_f64().is_some_and(f64::is_finite),
            Self::Boolean { .. } => value.is_boolean(),
            Self::Choice { options, .. } => value
                .as_str()
                .is_some_and(|text| options.iter().any(|option| option == text)),
            Self::Json { .. } => true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeAction {
    pub id: String,
    pub label: String,
    pub operation: NativeOperation,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum NativeOperation {
    #[serde(rename_all = "camelCase")]
    ExecuteCommand { command_id: String },
    #[serde(rename_all = "camelCase")]
    StartTask { task_type: String },
    #[serde(rename_all = "camelCase")]
    OpenView { view_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeCommandReply {
    pub message: String,
    pub view: Option<NativeView>,
}

impl NativeView {
    pub fn validate(&self, manifest: &PluginManifest) -> Result<(), PluginFailure> {
        let invalid = || PluginFailure::new("plugin_view_invalid");
        if self.description.len() > 16 * 1024
            || self.fields.len() > 64
            || self.actions.len() > 32
            || serde_json::to_vec(self).map_err(|_| invalid())?.len() as u64 > MAX_NATIVE_VIEW_BYTES
        {
            return Err(invalid());
        }
        let mut ids = BTreeSet::new();
        for field in &self.fields {
            if !valid_id(&field.id)
                || !ids.insert(&field.id)
                || !valid_label(&field.label)
                || !field.input.accepts(&field.input.value())
            {
                return Err(invalid());
            }
            if let NativeInput::Choice { options, .. } = &field.input
                && (options.is_empty()
                    || options.len() > 64
                    || options.iter().any(|option| !valid_label(option))
                    || options.iter().collect::<BTreeSet<_>>().len() != options.len())
            {
                return Err(invalid());
            }
        }
        ids.clear();
        for action in &self.actions {
            if !valid_id(&action.id) || !ids.insert(&action.id) || !valid_label(&action.label) {
                return Err(invalid());
            }
            let (method, declared) = match &action.operation {
                NativeOperation::ExecuteCommand { command_id } => (
                    "commands.execute",
                    manifest
                        .contributes
                        .commands
                        .iter()
                        .any(|item| item.id == *command_id),
                ),
                NativeOperation::StartTask { task_type } => (
                    "tasks.start",
                    manifest
                        .contributes
                        .task_types
                        .iter()
                        .any(|item| item.id == *task_type),
                ),
                NativeOperation::OpenView { view_id } => (
                    "views.open",
                    manifest
                        .contributes
                        .views
                        .iter()
                        .any(|item| item.id == *view_id),
                ),
            };
            if !declared || !manifest.ui_methods.iter().any(|allowed| allowed == method) {
                return Err(PluginFailure::new("plugin_method_denied"));
            }
            if matches!(action.operation, NativeOperation::StartTask { .. })
                && !manifest
                    .ui_methods
                    .iter()
                    .any(|allowed| allowed == "tasks.get")
            {
                return Err(PluginFailure::new("plugin_method_denied"));
            }
        }
        Ok(())
    }
}

impl NativeCommandReply {
    pub fn validate(&self, manifest: &PluginManifest) -> Result<(), PluginFailure> {
        if self.message.len() > 64 * 1024 {
            return Err(PluginFailure::new("plugin_view_invalid"));
        }
        if let Some(view) = &self.view {
            view.validate(manifest)?;
        }
        Ok(())
    }
}

fn valid_label(text: &str) -> bool {
    !text.is_empty() && text.len() <= 128
}
