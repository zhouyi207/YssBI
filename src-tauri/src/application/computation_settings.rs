use crate::execution::settings::{
    ExecutionMissingValuePolicy, ExecutionNumericTolerance, ExecutionSettings,
};
use crate::project::{
    ComputationSettingsValidationError, ProjectComputationSettings, StatisticalMissingValuePolicy,
};
use crate::sci::api::computation::{MissingValuePolicy, NumericTolerance, SciComputationSettings};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ComputationSettingsMappingError {
    #[error("numeric tolerance is invalid")]
    InvalidTolerance,
    #[error("numeric tolerance cannot be zero in both dimensions")]
    ZeroTolerance,
}

pub fn sci_computation_settings(
    project: &ProjectComputationSettings,
) -> Result<SciComputationSettings, ComputationSettingsMappingError> {
    validate(project)?;
    let missing_values = match project.missing_values.statistics {
        StatisticalMissingValuePolicy::Listwise => MissingValuePolicy::Listwise,
        StatisticalMissingValuePolicy::Reject => MissingValuePolicy::Reject,
    };
    Ok(SciComputationSettings {
        tolerance: NumericTolerance {
            absolute: project.numeric.tolerance.absolute,
            relative: project.numeric.tolerance.relative,
        },
        missing_values,
    })
}

pub fn execution_settings(
    project: &ProjectComputationSettings,
) -> Result<ExecutionSettings, ComputationSettingsMappingError> {
    validate(project)?;
    let statistical_missing_values = match project.missing_values.statistics {
        StatisticalMissingValuePolicy::Listwise => ExecutionMissingValuePolicy::Listwise,
        StatisticalMissingValuePolicy::Reject => ExecutionMissingValuePolicy::Reject,
    };
    Ok(ExecutionSettings {
        numeric_tolerance: ExecutionNumericTolerance {
            absolute: project.numeric.tolerance.absolute,
            relative: project.numeric.tolerance.relative,
        },
        statistical_missing_values,
    })
}

fn validate(project: &ProjectComputationSettings) -> Result<(), ComputationSettingsMappingError> {
    project.validate().map_err(|error| match error {
        ComputationSettingsValidationError::InvalidTolerance => {
            ComputationSettingsMappingError::InvalidTolerance
        }
        ComputationSettingsValidationError::ZeroTolerance => {
            ComputationSettingsMappingError::ZeroTolerance
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{ComputationSettingsMappingError, execution_settings, sci_computation_settings};
    use crate::execution::settings::{
        ExecutionMissingValuePolicy, ExecutionNumericTolerance, ExecutionSettings,
    };
    use crate::project::{
        MissingValueSettings, NumericSettings, NumericTolerance, ProjectComputationSettings,
        StatisticalMissingValuePolicy,
    };
    use crate::sci::api::computation::{
        MissingValuePolicy, NumericTolerance as SciNumericTolerance, SciComputationSettings,
    };

    #[test]
    fn project_settings_map_independently_to_sci_and_execution_contracts() {
        let project = ProjectComputationSettings {
            numeric: NumericSettings {
                tolerance: NumericTolerance {
                    absolute: 0.25,
                    relative: 0.5,
                },
            },
            missing_values: MissingValueSettings {
                statistics: StatisticalMissingValuePolicy::Reject,
            },
        };

        assert_eq!(
            sci_computation_settings(&project),
            Ok(SciComputationSettings {
                tolerance: SciNumericTolerance {
                    absolute: 0.25,
                    relative: 0.5,
                },
                missing_values: MissingValuePolicy::Reject,
            })
        );
        assert_eq!(
            execution_settings(&project),
            Ok(ExecutionSettings {
                numeric_tolerance: ExecutionNumericTolerance {
                    absolute: 0.25,
                    relative: 0.5,
                },
                statistical_missing_values: ExecutionMissingValuePolicy::Reject,
            })
        );
    }

    #[test]
    fn project_tolerance_validation_maps_to_closed_application_errors() {
        let cases = [
            (
                NumericTolerance {
                    absolute: f64::NAN,
                    relative: 1.0,
                },
                ComputationSettingsMappingError::InvalidTolerance,
            ),
            (
                NumericTolerance {
                    absolute: 0.0,
                    relative: 0.0,
                },
                ComputationSettingsMappingError::ZeroTolerance,
            ),
        ];

        for (tolerance, expected) in cases {
            let project = ProjectComputationSettings {
                numeric: NumericSettings { tolerance },
                ..ProjectComputationSettings::default()
            };
            assert_eq!(sci_computation_settings(&project), Err(expected));
            assert_eq!(execution_settings(&project), Err(expected));
        }
    }
}
