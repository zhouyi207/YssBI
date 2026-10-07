//! Explicit project lifecycle inputs. Concurrency remains in the internal request.
use crate::{DatasetImportSource, ProjectResourceRef, ResourceCreation};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CreateResourceInput {
    EventGraph { name: String },
    FunctionGraph { name: String },
    Chart { name: String },
    Mind { name: String },
    Doc { name: String },
}

impl From<CreateResourceInput> for ResourceCreation {
    fn from(value: CreateResourceInput) -> Self {
        match value {
            CreateResourceInput::EventGraph { name } => Self::EventGraph { name },
            CreateResourceInput::FunctionGraph { name } => Self::FunctionGraph { name },
            CreateResourceInput::Chart { name } => Self::Chart { name },
            CreateResourceInput::Mind { name } => Self::Mind { name },
            CreateResourceInput::Doc { name } => Self::Doc { name },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportDatabaseInput {
    pub source: DatasetImportSource,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenameResourceInput {
    pub resource: ProjectResourceRef,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceTargetInput {
    pub resource: ProjectResourceRef,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DuplicateResourceInput {
    pub resource: ProjectResourceRef,
    #[serde(default)]
    pub name: Option<String>,
}
