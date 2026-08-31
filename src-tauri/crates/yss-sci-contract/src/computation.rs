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

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum StatisticalInputValidationError {
    #[error("statistical input name is blank")]
    BlankName,
    #[error("statistical input contains a non-finite numeric value")]
    NonFiniteNumeric { index: usize },
}

impl StatisticalInput {
    pub fn try_new(
        name: Box<str>,
        values: Box<[Option<StatisticalScalar>]>,
        categorical_role: Option<CategoricalRole>,
    ) -> Result<Self, StatisticalInputValidationError> {
        if name.trim().is_empty() {
            return Err(StatisticalInputValidationError::BlankName);
        }
        if let Some(index) = values.iter().position(
            |value| matches!(value, Some(StatisticalScalar::Numeric(number)) if !number.is_finite()),
        ) {
            return Err(StatisticalInputValidationError::NonFiniteNumeric { index });
        }
        Ok(Self {
            name,
            values,
            categorical_role,
        })
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

#[cfg(test)]
mod tests {
    use super::{
        CategoricalRole, MissingValuePolicy, StatisticalInput, StatisticalInputValidationError,
        StatisticalObservationMetadata, StatisticalScalar, StatisticalSettingSource,
    };
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

    #[test]
    fn statistical_inputs_can_only_be_constructed_from_named_finite_values() {
        assert_eq!(
            StatisticalInput::try_new("  ".into(), Box::new([]), None),
            Err(StatisticalInputValidationError::BlankName)
        );
        assert_eq!(
            StatisticalInput::try_new(
                "series".into(),
                Box::new([Some(StatisticalScalar::Numeric(f64::INFINITY))]),
                None,
            ),
            Err(StatisticalInputValidationError::NonFiniteNumeric { index: 0 })
        );

        let input = StatisticalInput::try_new(
            "series".into(),
            Box::new([
                None,
                Some(StatisticalScalar::Numeric(2.5)),
                Some(StatisticalScalar::Category("group-a".into())),
            ]),
            Some(CategoricalRole::Time),
        )
        .expect("named finite statistical input must be constructible");
        assert_eq!(input.name(), "series");
        assert_eq!(input.categorical_role(), Some(CategoricalRole::Time));
    }
}
