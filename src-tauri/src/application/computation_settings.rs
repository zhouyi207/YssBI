use super::execution::session_slot::{
    ApplicationSessionRefreshError, ApplicationState, SessionCaptureError, SessionRevalidationError,
};
use crate::project::{
    ComputationSettingsMutationReceipt, ComputationSettingsMutationRequest,
    ComputationSettingsSnapshot, ProjectFilesystemError, ProjectInstanceId,
};
use crate::project::{
    ComputationSettingsValidationError, ProjectComputationSettings, StatisticalMissingValuePolicy,
};
use crate::sci::api::computation::{MissingValuePolicy, NumericTolerance, SciComputationSettings};
use yss_execution::settings::{
    ExecutionMissingValuePolicy, ExecutionNumericTolerance, ExecutionSettings,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ComputationSettingsMappingError {
    #[error("numeric tolerance is invalid")]
    InvalidTolerance,
    #[error("numeric tolerance cannot be zero in both dimensions")]
    ZeroTolerance,
}

#[derive(Debug, thiserror::Error)]
pub enum ComputationSettingsApplicationError {
    #[error(transparent)]
    SessionCapture(#[from] SessionCaptureError),
    #[error("computation-settings request belongs to another project instance")]
    ProjectIdentityMismatch { requested: ProjectInstanceId },
    #[error(transparent)]
    Project(#[from] ProjectFilesystemError),
    #[error(transparent)]
    Mapping(#[from] ComputationSettingsMappingError),
    #[error("captured application session changed during computation-settings operation")]
    SessionChanged(#[source] SessionRevalidationError),
    #[error("application session refresh failed")]
    SessionRefresh(#[source] ApplicationSessionRefreshError),
}

impl ApplicationState {
    pub fn query_computation_settings(
        &self,
        project_instance_id: ProjectInstanceId,
    ) -> Result<ComputationSettingsSnapshot, ComputationSettingsApplicationError> {
        let captured = self.capture_session()?;
        if captured.project_instance_id() != &project_instance_id {
            return Err(
                ComputationSettingsApplicationError::ProjectIdentityMismatch {
                    requested: project_instance_id,
                },
            );
        }
        let result = captured.project().get_computation_settings()?;
        self.revalidate_captured_session(&captured)
            .map_err(ComputationSettingsApplicationError::SessionChanged)?;
        Ok(result)
    }

    pub fn update_computation_settings(
        &self,
        request: ComputationSettingsMutationRequest,
    ) -> Result<ComputationSettingsMutationReceipt, ComputationSettingsApplicationError> {
        let captured = self.capture_session()?;
        if captured.project_instance_id() != &request.project_instance_id {
            return Err(
                ComputationSettingsApplicationError::ProjectIdentityMismatch {
                    requested: request.project_instance_id,
                },
            );
        }
        let _ = execution_settings(&request.settings)?;
        let result = captured
            .project()
            .update_computation_settings_transaction(request)?;
        self.refresh_current_project()
            .map_err(ComputationSettingsApplicationError::SessionRefresh)?;
        Ok(result)
    }
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
    use crate::project::{
        MissingValueSettings, NumericSettings, NumericTolerance, ProjectComputationSettings,
        StatisticalMissingValuePolicy,
    };
    use crate::sci::api::computation::{
        MissingValuePolicy, NumericTolerance as SciNumericTolerance, SciComputationSettings,
    };
    use yss_execution::settings::{
        ExecutionMissingValuePolicy, ExecutionNumericTolerance, ExecutionSettings,
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
