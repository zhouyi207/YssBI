use serde::{Deserialize, Serialize};

pub const CURRENT_PROJECT_SCHEMA_VERSION: u32 = 5;

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
    #[serde(deserialize_with = "deserialize_export_time")]
    export_time: String,
}

fn normalize_export_time(value: String) -> String {
    chrono::DateTime::parse_from_rfc3339(&value).map_or(value, |time| {
        time.naive_local()
            .format("%Y-%m-%dT%H:%M:%S%.f")
            .to_string()
    })
}

fn deserialize_export_time<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<String, D::Error> {
    String::deserialize(deserializer).map(normalize_export_time)
}

impl ProjectManifest {
    pub fn new(project_name: impl Into<String>, export_time: impl Into<String>) -> Self {
        Self {
            schema_version: CURRENT_PROJECT_SCHEMA_VERSION,
            project_name: project_name.into(),
            export_time: normalize_export_time(export_time.into()),
        }
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
    fn manifest_times_keep_the_input_calendar_clock_without_offset() {
        let input = "2026-09-11T10:00:00.123+08:00";
        let manifest = ProjectManifest::new("Clock", input);
        assert_eq!(manifest.export_time, "2026-09-11T10:00:00.123");
        let loaded: ProjectManifest = serde_json::from_value(json!({
            "schemaVersion": CURRENT_PROJECT_SCHEMA_VERSION, "projectName": "Clock", "exportTime": input,
        })).unwrap();
        assert_eq!(loaded, manifest);
    }

    #[test]
    fn constructor_mints_only_the_current_project_schema_version() {
        let manifest = ProjectManifest::new("Example", "2026-08-30T00:00:00");

        assert_eq!(
            serde_json::to_value(&manifest).unwrap(),
            json!({
                "schemaVersion": CURRENT_PROJECT_SCHEMA_VERSION,
                "projectName": "Example",
                "exportTime": "2026-08-30T00:00:00"
            })
        );
        assert_eq!(manifest.schema_version, CURRENT_PROJECT_SCHEMA_VERSION);
    }

    #[test]
    fn deserialization_rejects_non_current_schema_versions() {
        let mut value = json!({
            "schemaVersion": CURRENT_PROJECT_SCHEMA_VERSION,
            "projectName": "Example",
            "exportTime": "2026-08-30T00:00:00"
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
            "exportTime": "2026-08-30T00:00:00",
            "computationSettings": {
                "numeric": { "tolerance": { "absolute": 1e-12, "relative": 1e-9 } },
                "missingValues": { "statistics": "listwise" }
            }
        });
        let manifest = serde_json::from_value::<ProjectManifest>(value).unwrap();
        assert_eq!(manifest.project_name, "Example");
    }

    #[test]
    fn validated_manifest_parts_round_trip_without_public_mutation_seams() {
        let manifest = ProjectManifest::new("Round Trip", "2026-08-30T00:00:00");

        assert_eq!(manifest.project_name, "Round Trip");
        assert_eq!(manifest.export_time, "2026-08-30T00:00:00");
        assert_eq!(
            manifest.into_parts(),
            ("Round Trip".to_owned(), "2026-08-30T00:00:00".to_owned())
        );
    }
}
