use crate::{ChartSettings, ResourceVersion, model::ChartResourceRef};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectChartRequest {
    pub chart: ChartResourceRef,
    pub version: Option<ResourceVersion>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChartInspection {
    pub chart: ChartResourceRef,
    pub version: ResourceVersion,
    pub settings: ChartSettings,
}
