use super::execution::session_slot::{
    ApplicationSessionRefreshError, ApplicationState, SessionCaptureError, SessionRevalidationError,
};
use crate::sci::api::computation::{MissingValuePolicy, NumericTolerance, SciComputationSettings};
use yss_computation_settings::{
    ComputationSettingsMutationReceipt, ComputationSettingsMutationRequest,
    ComputationSettingsSnapshot, ComputationSettingsValidationError, ProjectComputationSettings,
    StatisticalMissingValuePolicy,
};
use yss_execution::settings::{
    ExecutionMissingValuePolicy, ExecutionNumericTolerance, ExecutionSettings,
};
use yss_project_filesystem::ProjectFilesystemError;
use yss_project_identity::ProjectInstanceId;

#[derive(Debug, thiserror::Error)]
pub enum ComputationSettingsApplicationError {
    #[error(transparent)]
    SessionCapture(#[from] SessionCaptureError),
    #[error("computation-settings request belongs to another project instance")]
    ProjectIdentityMismatch { requested: ProjectInstanceId },
    #[error(transparent)]
    Project(#[from] ProjectFilesystemError),
    #[error(transparent)]
    Validation(#[from] ComputationSettingsValidationError),
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
) -> Result<SciComputationSettings, ComputationSettingsValidationError> {
    project.validate()?;
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
) -> Result<ExecutionSettings, ComputationSettingsValidationError> {
    project.validate()?;
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

#[cfg(test)]
mod tests {
    use super::{execution_settings, sci_computation_settings};
    use crate::sci::api::computation::{
        MissingValuePolicy, NumericTolerance as SciNumericTolerance, SciComputationSettings,
    };
    use yss_computation_settings::{
        ComputationSettingsValidationError, MissingValueSettings, NumericSettings,
        NumericTolerance, ProjectComputationSettings, StatisticalMissingValuePolicy,
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
                ComputationSettingsValidationError::InvalidTolerance,
            ),
            (
                NumericTolerance {
                    absolute: 0.0,
                    relative: 0.0,
                },
                ComputationSettingsValidationError::ZeroTolerance,
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
