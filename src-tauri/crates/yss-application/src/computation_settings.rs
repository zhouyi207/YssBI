//! Application adapters for the global computation settings authority.

use yss_execution::settings::{
    ExecutionMissingValuePolicy, ExecutionNumericTolerance, ExecutionSettings,
};
use yss_sci_contract::{MissingValuePolicy, NumericTolerance, SciComputationSettings};
use yss_settings::{
    ComputationSettings, ComputationSettingsValidationError, StatisticalMissingValuePolicy,
};

pub fn sci_computation_settings(
    settings: &ComputationSettings,
) -> Result<SciComputationSettings, ComputationSettingsValidationError> {
    settings.validate()?;
    let missing_values = match settings.missing_values.statistics {
        StatisticalMissingValuePolicy::Listwise => MissingValuePolicy::Listwise,
        StatisticalMissingValuePolicy::Reject => MissingValuePolicy::Reject,
    };
    Ok(SciComputationSettings {
        tolerance: NumericTolerance {
            absolute: settings.numeric.tolerance.absolute,
            relative: settings.numeric.tolerance.relative,
        },
        missing_values,
    })
}

pub fn execution_settings(
    settings: &ComputationSettings,
) -> Result<ExecutionSettings, ComputationSettingsValidationError> {
    settings.validate()?;
    let statistical_missing_values = match settings.missing_values.statistics {
        StatisticalMissingValuePolicy::Listwise => ExecutionMissingValuePolicy::Listwise,
        StatisticalMissingValuePolicy::Reject => ExecutionMissingValuePolicy::Reject,
    };
    Ok(ExecutionSettings {
        numeric_tolerance: ExecutionNumericTolerance {
            absolute: settings.numeric.tolerance.absolute,
            relative: settings.numeric.tolerance.relative,
        },
        statistical_missing_values,
    })
}

#[cfg(test)]
mod tests {
    use super::{execution_settings, sci_computation_settings};
    use yss_execution::settings::{
        ExecutionMissingValuePolicy, ExecutionNumericTolerance, ExecutionSettings,
    };
    use yss_sci_contract::{
        MissingValuePolicy, NumericTolerance as SciNumericTolerance, SciComputationSettings,
    };
    use yss_settings::{
        ComputationSettings, ComputationSettingsValidationError, MissingValueSettings,
        NumericSettings, NumericTolerance, StatisticalMissingValuePolicy,
    };

    #[test]
    fn global_settings_map_to_sci_and_execution_contracts() {
        let settings = ComputationSettings {
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
            sci_computation_settings(&settings),
            Ok(SciComputationSettings {
                tolerance: SciNumericTolerance {
                    absolute: 0.25,
                    relative: 0.5,
                },
                missing_values: MissingValuePolicy::Reject,
            })
        );
        assert_eq!(
            execution_settings(&settings),
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
    fn global_tolerance_validation_is_shared_by_both_adapters() {
        let settings = ComputationSettings {
            numeric: NumericSettings {
                tolerance: NumericTolerance {
                    absolute: 0.0,
                    relative: 0.0,
                },
            },
            ..ComputationSettings::default()
        };
        assert_eq!(
            sci_computation_settings(&settings),
            Err(ComputationSettingsValidationError::ZeroTolerance)
        );
        assert_eq!(
            execution_settings(&settings),
            Err(ComputationSettingsValidationError::ZeroTolerance)
        );
    }
}
