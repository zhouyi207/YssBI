use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Resource categories shared by project automation and workbench intents.
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ProjectResourceKind {
    EventGraph,
    FunctionGraph,
    Chart,
    Mind,
    Doc,
    Database,
}

/// IDs are opaque. Each resource owner validates its own path or database identity.
#[derive(
    Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectResourceRef {
    pub kind: ProjectResourceKind,
    pub id: String,
}
