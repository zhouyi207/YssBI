//! Application-scoped settings authority.
//!
//! Frontend presentation preferences remain owned by the React settings store.
//! This crate owns backend runtime settings and their durable revisioned store.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use yss_project_identity::OperationId;

pub const SETTINGS_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_ABSOLUTE_TOLERANCE: f64 = 1e-12;
pub const DEFAULT_RELATIVE_TOLERANCE: f64 = 1e-9;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StatisticalMissingValuePolicy {
    #[default]
    Listwise,
    Reject,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NumericSettings {
    pub tolerance: NumericTolerance,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MissingValueSettings {
    pub statistics: StatisticalMissingValuePolicy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ComputationSettings {
    pub numeric: NumericSettings,
    pub missing_values: MissingValueSettings,
}

impl ComputationSettings {
    pub fn validate(&self) -> Result<(), ComputationSettingsValidationError> {
        self.numeric.tolerance.validate()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ComputationSettingsValidationError {
    #[error("numeric tolerances must be finite and nonnegative")]
    InvalidTolerance,
    #[error("absolute and relative tolerances cannot both be zero")]
    ZeroTolerance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplicationSettings {
    pub computation: ComputationSettings,
}

impl ApplicationSettings {
    pub fn validate(&self) -> Result<(), ComputationSettingsValidationError> {
        self.computation.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsSnapshot {
    pub settings_revision: u64,
    pub settings: ApplicationSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsMutationRequest {
    pub operation_id: OperationId,
    pub expected_revision: u64,
    pub settings: ApplicationSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsMutationReceipt {
    pub operation_id: OperationId,
    pub settings_revision: u64,
    pub settings: ApplicationSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PersistedSettings {
    schema_version: u32,
    settings_revision: u64,
    settings: ApplicationSettings,
}

impl Default for PersistedSettings {
    fn default() -> Self {
        Self {
            schema_version: SETTINGS_SCHEMA_VERSION,
            settings_revision: 0,
            settings: ApplicationSettings::default(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsStoreError {
    #[error("settings file I/O failed")]
    Io(#[source] std::io::Error),
    #[error("settings file could not be decoded")]
    Deserialize(#[source] serde_json::Error),
    #[error("settings file could not be encoded")]
    Serialize(#[source] serde_json::Error),
    #[error("unsupported settings schema version {actual}; expected {expected}")]
    UnsupportedSchema { actual: u32, expected: u32 },
    #[error(transparent)]
    Validation(#[from] ComputationSettingsValidationError),
    #[error("settings revision conflict: expected {expected}, current {current}")]
    RevisionConflict { expected: u64, current: u64 },
    #[error("settings revision is exhausted")]
    RevisionExhausted,
}

pub struct SettingsStore {
    path: PathBuf,
    state: Mutex<PersistedSettings>,
}

impl SettingsStore {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, SettingsStoreError> {
        let path = path.into();
        let state = if path.exists() {
            let bytes = std::fs::read(&path).map_err(SettingsStoreError::Io)?;
            serde_json::from_slice::<PersistedSettings>(&bytes)
                .map_err(SettingsStoreError::Deserialize)?
        } else {
            PersistedSettings::default()
        };
        if state.schema_version != SETTINGS_SCHEMA_VERSION {
            return Err(SettingsStoreError::UnsupportedSchema {
                actual: state.schema_version,
                expected: SETTINGS_SCHEMA_VERSION,
            });
        }
        state.settings.validate()?;
        Ok(Self {
            path,
            state: Mutex::new(state),
        })
    }

    pub fn snapshot(&self) -> SettingsSnapshot {
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        SettingsSnapshot {
            settings_revision: state.settings_revision,
            settings: state.settings.clone(),
        }
    }

    pub fn update(
        &self,
        request: SettingsMutationRequest,
    ) -> Result<SettingsMutationReceipt, SettingsStoreError> {
        request.settings.validate()?;
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.settings_revision != request.expected_revision {
            return Err(SettingsStoreError::RevisionConflict {
                expected: request.expected_revision,
                current: state.settings_revision,
            });
        }
        let settings_revision = state
            .settings_revision
            .checked_add(1)
            .ok_or(SettingsStoreError::RevisionExhausted)?;
        let next = PersistedSettings {
            schema_version: SETTINGS_SCHEMA_VERSION,
            settings_revision,
            settings: request.settings.clone(),
        };
        self.persist(&next)?;
        *state = next;
        Ok(SettingsMutationReceipt {
            operation_id: request.operation_id,
            settings_revision,
            settings: request.settings,
        })
    }

    fn persist(&self, state: &PersistedSettings) -> Result<(), SettingsStoreError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(SettingsStoreError::Io)?;
        }
        let bytes = serde_json::to_vec_pretty(state).map_err(SettingsStoreError::Serialize)?;
        std::fs::write(&self.path, bytes).map_err(SettingsStoreError::Io)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!("yssbi-settings-{label}-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn global_settings_round_trip_with_revisioned_update() {
        let path = temporary_path("round-trip");
        let store = SettingsStore::open(&path).unwrap();
        let mut settings = ApplicationSettings::default();
        settings.computation.numeric.tolerance.absolute = 0.25;
        let receipt = store
            .update(SettingsMutationRequest {
                operation_id: OperationId::new(),
                expected_revision: 0,
                settings: settings.clone(),
            })
            .unwrap();
        assert_eq!(receipt.settings_revision, 1);
        assert_eq!(store.snapshot().settings, settings);

        let reopened = SettingsStore::open(&path).unwrap();
        assert_eq!(reopened.snapshot().settings_revision, 1);
        assert_eq!(reopened.snapshot().settings, settings);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn global_settings_reject_stale_revision_and_zero_tolerance() {
        let path = temporary_path("validation");
        let store = SettingsStore::open(&path).unwrap();
        let stale = store.update(SettingsMutationRequest {
            operation_id: OperationId::new(),
            expected_revision: 1,
            settings: ApplicationSettings::default(),
        });
        assert!(matches!(
            stale,
            Err(SettingsStoreError::RevisionConflict { .. })
        ));

        let mut invalid = ApplicationSettings::default();
        invalid.computation.numeric.tolerance = NumericTolerance {
            absolute: 0.0,
            relative: 0.0,
        };
        let result = store.update(SettingsMutationRequest {
            operation_id: OperationId::new(),
            expected_revision: 0,
            settings: invalid,
        });
        assert!(matches!(result, Err(SettingsStoreError::Validation(_))));
        let _ = std::fs::remove_file(path);
    }
}
