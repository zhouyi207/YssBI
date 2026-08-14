use serde::{Deserialize, Serialize};

pub const DEFAULT_ABSOLUTE_TOLERANCE: f64 = 1e-12;
pub const DEFAULT_RELATIVE_TOLERANCE: f64 = 1e-9;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NumericTolerance {
    pub absolute: f64,
    pub relative: f64,
}

impl NumericTolerance {
    pub fn validate(&self) -> Result<(), ComputationSettingsValidationError> {
        if !self.absolute.is_finite()
            || !self.relative.is_finite()
            || self.absolute < 0.0
            || self.relative < 0.0
        {
            return Err(ComputationSettingsValidationError::InvalidTolerance);
        }
        if self.absolute == 0.0 && self.relative == 0.0 {
            return Err(ComputationSettingsValidationError::ZeroTolerance);
        }
        Ok(())
    }
}

impl Default for NumericTolerance {
    fn default() -> Self {
        Self {
            absolute: DEFAULT_ABSOLUTE_TOLERANCE,
            relative: DEFAULT_RELATIVE_TOLERANCE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StatisticalMissingValuePolicy {
    Listwise,
    Reject,
}

impl Default for StatisticalMissingValuePolicy {
    fn default() -> Self {
        Self::Listwise
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NumericSettings {
    pub tolerance: NumericTolerance,
}

impl Default for NumericSettings {
    fn default() -> Self {
        Self {
            tolerance: NumericTolerance::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingValueSettings {
    pub statistics: StatisticalMissingValuePolicy,
}

impl Default for MissingValueSettings {
    fn default() -> Self {
        Self {
            statistics: StatisticalMissingValuePolicy::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProjectComputationSettings {
    pub numeric: NumericSettings,
    pub missing_values: MissingValueSettings,
}

impl ProjectComputationSettings {
    pub fn validate(&self) -> Result<(), ComputationSettingsValidationError> {
        self.numeric.tolerance.validate()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputationSettingsValidationError {
    InvalidTolerance,
    ZeroTolerance,
}

impl std::fmt::Display for ComputationSettingsValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTolerance => {
                formatter.write_str("numeric tolerances must be finite and nonnegative")
            }
            Self::ZeroTolerance => {
                formatter.write_str("absolute and relative tolerances cannot both be zero")
            }
        }
    }
}

impl std::error::Error for ComputationSettingsValidationError {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ComputationSettingsSnapshot {
    pub project_instance_id: crate::project::ProjectInstanceId,
    pub settings_revision: u64,
    pub publication_revision: u64,
    pub settings: ProjectComputationSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ComputationSettingsMutationRequest {
    pub project_instance_id: crate::project::ProjectInstanceId,
    pub operation_id: crate::node_system::document::OperationId,
    pub expected_revision: u64,
    pub settings: ProjectComputationSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ComputationSettingsMutationReceipt {
    pub project_instance_id: crate::project::ProjectInstanceId,
    pub operation_id: crate::node_system::document::OperationId,
    pub settings_revision: u64,
    pub publication_revision: u64,
    pub settings: ProjectComputationSettings,
}
