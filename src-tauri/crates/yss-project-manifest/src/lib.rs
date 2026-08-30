use serde::{Deserialize, Serialize};
use yss_computation_settings::{ComputationSettingsValidationError, ProjectComputationSettings};

pub const CURRENT_PROJECT_SCHEMA_VERSION: u32 = 3;

pub fn deserialize_current_project_schema_version<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let schema_version = u32::deserialize(deserializer)?;
    if schema_version != CURRENT_PROJECT_SCHEMA_VERSION {
        return Err(serde::de::Error::custom(format!(
            "unsupported schema version {schema_version}; expected {CURRENT_PROJECT_SCHEMA_VERSION}"
        )));
    }
    Ok(schema_version)
}

fn deserialize_valid_computation_settings<'de, D>(
    deserializer: D,
) -> Result<ProjectComputationSettings, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let settings = ProjectComputationSettings::deserialize(deserializer)?;
    settings.validate().map_err(serde::de::Error::custom)?;
    Ok(settings)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectManifest {
    #[serde(deserialize_with = "deserialize_current_project_schema_version")]
    schema_version: u32,
    project_name: String,
    export_time: String,
    #[serde(deserialize_with = "deserialize_valid_computation_settings")]
    computation_settings: ProjectComputationSettings,
}

impl ProjectManifest {
    pub fn try_new(
        project_name: impl Into<String>,
        export_time: impl Into<String>,
        computation_settings: ProjectComputationSettings,
    ) -> Result<Self, ComputationSettingsValidationError> {
        computation_settings.validate()?;
        Ok(Self {
            schema_version: CURRENT_PROJECT_SCHEMA_VERSION,
            project_name: project_name.into(),
            export_time: export_time.into(),
            computation_settings,
        })
    }

    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn project_name(&self) -> &str {
        &self.project_name
    }

    pub fn export_time(&self) -> &str {
        &self.export_time
    }

    pub fn computation_settings(&self) -> &ProjectComputationSettings {
        &self.computation_settings
    }

    pub fn into_parts(self) -> (String, String, ProjectComputationSettings) {
        (
            self.project_name,
            self.export_time,
            self.computation_settings,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn manifest_with(settings: serde_json::Value) -> serde_json::Value {
        json!({
            "schemaVersion": CURRENT_PROJECT_SCHEMA_VERSION,
            "projectName": "Computation Settings",
            "exportTime": "2026-08-30T00:00:00Z",
            "computationSettings": settings
        })
    }

    #[test]
    fn constructor_mints_only_the_current_project_schema_version() {
        let manifest = ProjectManifest::try_new(
            "Example",
            "2026-08-30T00:00:00Z",
            ProjectComputationSettings::default(),
        )
        .unwrap();

        assert_eq!(
            serde_json::to_value(&manifest).unwrap(),
            json!({
                "schemaVersion": CURRENT_PROJECT_SCHEMA_VERSION,
                "projectName": "Example",
                "exportTime": "2026-08-30T00:00:00Z",
                "computationSettings": ProjectComputationSettings::default()
            })
        );
        assert_eq!(manifest.schema_version(), CURRENT_PROJECT_SCHEMA_VERSION);
    }

    #[test]
    fn deserialization_rejects_non_current_schema_versions() {
        let mut value =
            manifest_with(serde_json::to_value(ProjectComputationSettings::default()).unwrap());

        for schema_version in [
            CURRENT_PROJECT_SCHEMA_VERSION - 1,
            CURRENT_PROJECT_SCHEMA_VERSION + 1,
        ] {
            value["schemaVersion"] = json!(schema_version);
            let error = serde_json::from_value::<ProjectManifest>(value.clone()).unwrap_err();
            assert!(error.to_string().contains("unsupported schema version"));
        }
    }

    #[test]
    fn construction_and_deserialization_reject_invalid_computation_settings() {
        let invalid_settings: ProjectComputationSettings = serde_json::from_value(json!({
            "numeric": { "tolerance": { "absolute": 0.0, "relative": 0.0 } },
            "missingValues": { "statistics": "listwise" }
        }))
        .unwrap();

        assert!(
            ProjectManifest::try_new("Invalid", "2026-08-30T00:00:00Z", invalid_settings.clone(),)
                .is_err()
        );
        assert!(
            serde_json::from_value::<ProjectManifest>(manifest_with(
                serde_json::to_value(invalid_settings).unwrap(),
            ))
            .is_err()
        );

        let unknown_nested_field = json!({
            "numeric": {
                "tolerance": { "absolute": 1e-12, "relative": 1e-9 },
                "legacyTolerance": 1.0
            },
            "missingValues": { "statistics": "listwise" }
        });
        assert!(
            serde_json::from_value::<ProjectManifest>(manifest_with(unknown_nested_field)).is_err()
        );
    }

    #[test]
    fn validated_manifest_parts_round_trip_without_public_mutation_seams() {
        let settings = ProjectComputationSettings::default();
        let manifest =
            ProjectManifest::try_new("Round Trip", "2026-08-30T00:00:00Z", settings.clone())
                .unwrap();

        assert_eq!(manifest.project_name(), "Round Trip");
        assert_eq!(manifest.export_time(), "2026-08-30T00:00:00Z");
        assert_eq!(manifest.computation_settings(), &settings);
        assert_eq!(
            manifest.into_parts(),
            (
                "Round Trip".to_owned(),
                "2026-08-30T00:00:00Z".to_owned(),
                settings,
            )
        );
    }
}
