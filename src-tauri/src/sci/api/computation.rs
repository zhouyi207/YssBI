use crate::data_contract::DataValue;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CategoricalRole {
    General,
    Individual,
    Time,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MissingValuePolicy {
    Listwise,
    Reject,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NumericTolerance {
    pub absolute: f64,
    pub relative: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SciComputationSettings {
    pub tolerance: NumericTolerance,
    pub missing_values: MissingValuePolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatisticalSettingSource {
    #[serde(rename = "project")]
    ProjectDefault,
    #[serde(rename = "node")]
    NodeOverride,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatisticalObservationMetadata {
    pub original_observation_count: usize,
    pub used_observation_count: usize,
    pub dropped_null_count: usize,
    pub dropped_nan_count: usize,
    pub missing_value_policy: MissingValuePolicy,
    pub missing_value_policy_source: StatisticalSettingSource,
    pub effective_convergence_tolerance: f64,
    pub convergence_tolerance_source: StatisticalSettingSource,
    pub convergence_tolerance_consumed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StatisticalScalar {
    Numeric(f64),
    Category(Box<str>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct StatisticalInput {
    name: Box<str>,
    values: Box<[Option<StatisticalScalar>]>,
    categorical_role: Option<CategoricalRole>,
}

impl StatisticalInput {
    pub(crate) fn new(
        name: Box<str>,
        values: Box<[Option<StatisticalScalar>]>,
        categorical_role: Option<CategoricalRole>,
    ) -> Self {
        Self {
            name,
            values,
            categorical_role,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn values(&self) -> &[Option<StatisticalScalar>] {
        &self.values
    }

    pub fn categorical_role(&self) -> Option<CategoricalRole> {
        self.categorical_role
    }
}

pub struct StatisticalInputSource<'a> {
    pub name: &'a str,
    pub values: &'a [Option<DataValue>],
    pub categorical_role: Option<CategoricalRole>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatisticalValueKind {
    Boolean,
    Array,
    Object,
    DataFrame,
    DataSeries,
    Struct,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum StatisticalInputMappingError {
    #[error("statistical input contains a non-finite numeric value")]
    NonFiniteNumeric { index: usize },
    #[error("statistical input contains an unsupported value kind")]
    UnsupportedValue {
        index: usize,
        kind: StatisticalValueKind,
    },
}

#[cfg(test)]
mod tests {
    use super::{MissingValuePolicy, StatisticalObservationMetadata, StatisticalSettingSource};
    use serde_json::json;

    #[test]
    fn observation_metadata_preserves_the_existing_nine_field_wire() {
        let metadata = StatisticalObservationMetadata {
            original_observation_count: 10,
            used_observation_count: 7,
            dropped_null_count: 2,
            dropped_nan_count: 1,
            missing_value_policy: MissingValuePolicy::Reject,
            missing_value_policy_source: StatisticalSettingSource::NodeOverride,
            effective_convergence_tolerance: 1e-7,
            convergence_tolerance_source: StatisticalSettingSource::ProjectDefault,
            convergence_tolerance_consumed: true,
        };

        assert_eq!(
            serde_json::to_value(metadata)
                .expect("statistical observation metadata must serialize"),
            json!({
                "originalObservationCount": 10,
                "usedObservationCount": 7,
                "droppedNullCount": 2,
                "droppedNanCount": 1,
                "missingValuePolicy": "reject",
                "missingValuePolicySource": "node",
                "effectiveConvergenceTolerance": 1e-7,
                "convergenceToleranceSource": "project",
                "convergenceToleranceConsumed": true
            })
        );
    }
}
