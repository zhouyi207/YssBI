use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectManifest {
    #[serde(deserialize_with = "deserialize_current_project_schema_version")]
    schema_version: u32,
    project_name: String,
    export_time: String,
}

impl ProjectManifest {
    pub fn try_new(project_name: impl Into<String>, export_time: impl Into<String>) -> Self {
        Self {
            schema_version: CURRENT_PROJECT_SCHEMA_VERSION,
            project_name: project_name.into(),
            export_time: export_time.into(),
        }
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

    pub fn into_parts(self) -> (String, String) {
        (self.project_name, self.export_time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn constructor_mints_only_the_current_project_schema_version() {
        let manifest = ProjectManifest::try_new("Example", "2026-08-30T00:00:00Z");

        assert_eq!(
            serde_json::to_value(&manifest).unwrap(),
            json!({
                "schemaVersion": CURRENT_PROJECT_SCHEMA_VERSION,
                "projectName": "Example",
                "exportTime": "2026-08-30T00:00:00Z"
            })
        );
        assert_eq!(manifest.schema_version(), CURRENT_PROJECT_SCHEMA_VERSION);
    }

    #[test]
    fn deserialization_rejects_non_current_schema_versions() {
        let mut value = json!({
            "schemaVersion": CURRENT_PROJECT_SCHEMA_VERSION,
            "projectName": "Example",
            "exportTime": "2026-08-30T00:00:00Z"
        });

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
    fn unknown_project_settings_are_ignored_by_the_manifest_boundary() {
        let value = json!({
            "schemaVersion": CURRENT_PROJECT_SCHEMA_VERSION,
            "projectName": "Example",
            "exportTime": "2026-08-30T00:00:00Z",
            "computationSettings": {
                "numeric": { "tolerance": { "absolute": 1e-12, "relative": 1e-9 } },
                "missingValues": { "statistics": "listwise" }
            }
        });
        let manifest = serde_json::from_value::<ProjectManifest>(value).unwrap();
        assert_eq!(manifest.project_name(), "Example");
    }

    #[test]
    fn validated_manifest_parts_round_trip_without_public_mutation_seams() {
        let manifest = ProjectManifest::try_new("Round Trip", "2026-08-30T00:00:00Z");

        assert_eq!(manifest.project_name(), "Round Trip");
        assert_eq!(manifest.export_time(), "2026-08-30T00:00:00Z");
        assert_eq!(
            manifest.into_parts(),
            ("Round Trip".to_owned(), "2026-08-30T00:00:00Z".to_owned())
        );
    }
}
