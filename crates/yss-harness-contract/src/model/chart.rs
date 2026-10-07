//! Explicit chart configuration tools; omitted settings retain their current value.
use crate::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChartResourceKind {
    Chart,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChartResourceRef {
    pub kind: ChartResourceKind,
    pub id: String,
}
impl ChartResourceRef {
    pub fn new(id: String) -> Self {
        Self {
            kind: ChartResourceKind::Chart,
            id,
        }
    }
    pub fn resource(&self) -> ProjectResourceRef {
        ProjectResourceRef {
            kind: ProjectResourceKind::Chart,
            id: self.id.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectChartInput {
    pub chart: ChartResourceRef,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateChartInput {
    pub chart: ChartResourceRef,
    pub settings: ChartSettingsUpdate,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChartSettingsUpdate {
    /// Omit to keep the source; an empty string disconnects it. Null is not a database identity.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    #[schemars(with = "String")]
    pub database_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    #[schemars(with = "ChartType")]
    pub chart_type: Option<ChartType>,
    /// Omit to retain the X axis; null clears it.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::graph::present_optional"
    )]
    pub x: Option<Option<String>>,
    /// Omit to retain the Y axis; null clears it.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::graph::present_optional"
    )]
    pub y: Option<Option<String>>,
}

fn present_value<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

impl ChartSettingsUpdate {
    pub(crate) fn validate(&self) -> Result<(), CapabilityContractError> {
        if self.database_id.is_none()
            && self.chart_type.is_none()
            && self.x.is_none()
            && self.y.is_none()
        {
            return Err(CapabilityContractError::InvalidField("settings"));
        }
        if let Some(id) = &self.database_id
            && !id.is_empty()
        {
            validate_resource_id("settings.databaseId", id)?;
        }
        for (field, axis) in [("settings.x", &self.x), ("settings.y", &self.y)] {
            if let Some(Some(column)) = axis {
                validate_resource_id(field, column)?;
            }
        }
        Ok(())
    }
}
