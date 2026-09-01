use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use yss_project_identity::ResourceRevision;
use yss_project_layout::{CHART_EXTENSION, CHARTS_DIR};
use yss_resource_naming::{ResourceName, ResourceNameValidationError};

pub const CURRENT_CHART_SCHEMA_VERSION: u32 = 3;

fn deserialize_current_schema_version<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let schema_version = u32::deserialize(deserializer)?;
    if schema_version != CURRENT_CHART_SCHEMA_VERSION {
        return Err(serde::de::Error::custom(format!(
            "unsupported schema version {schema_version}; expected {CURRENT_CHART_SCHEMA_VERSION}"
        )));
    }
    Ok(schema_version)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChartEncodings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChartDocument {
    #[serde(deserialize_with = "deserialize_current_schema_version")]
    schema_version: u32,
    pub revision: ResourceRevision,
    pub database_id: String,
    pub chart_type: String,
    pub encodings: ChartEncodings,
}

impl ChartDocument {
    pub fn new(database_id: impl Into<String>) -> Self {
        Self {
            schema_version: CURRENT_CHART_SCHEMA_VERSION,
            revision: ResourceRevision::INITIAL,
            database_id: database_id.into(),
            chart_type: "histogram".to_owned(),
            encodings: ChartEncodings { x: None, y: None },
        }
    }

    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ChartResourcePathError {
    #[error("chart resource path must be in the charts directory")]
    WrongDirectory,
    #[error("chart resource path must not be nested")]
    Nested,
    #[error("chart resource path must use the .yssbi-chart extension")]
    WrongExtension,
    #[error("chart resource path has an invalid name")]
    InvalidName(#[source] ResourceNameValidationError),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ChartResourcePath {
    value: String,
    display_name: ResourceName,
}

impl ChartResourcePath {
    pub fn parse(value: &str) -> Result<Self, ChartResourcePathError> {
        let relative = value
            .strip_prefix(&format!("{CHARTS_DIR}/"))
            .ok_or(ChartResourcePathError::WrongDirectory)?;
        if relative.contains('/') || relative.contains('\\') {
            return Err(ChartResourcePathError::Nested);
        }
        let stem = relative
            .strip_suffix(&format!(".{CHART_EXTENSION}"))
            .ok_or(ChartResourcePathError::WrongExtension)?;
        let display_name =
            ResourceName::parse(stem).map_err(ChartResourcePathError::InvalidName)?;

        Ok(Self {
            value: value.to_owned(),
            display_name,
        })
    }

    pub fn from_name(name: &ResourceName) -> Self {
        Self {
            value: format!("{CHARTS_DIR}/{}.{CHART_EXTENSION}", name.as_str()),
            display_name: name.clone(),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub fn display_name(&self) -> &ResourceName {
        &self.display_name
    }

    pub fn relative_path(&self) -> &Path {
        Path::new(&self.value)
    }
}

impl TryFrom<String> for ChartResourcePath {
    type Error = ChartResourcePathError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<ChartResourcePath> for String {
    fn from(value: ChartResourcePath) -> Self {
        value.value
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::json;
    use yss_resource_naming::{ResourceName, ResourceNameValidationError};

    use super::{
        CURRENT_CHART_SCHEMA_VERSION, ChartDocument, ChartResourcePath, ChartResourcePathError,
    };

    #[test]
    fn chart_path_round_trips_canonical_resource_identity() {
        let name = ResourceName::parse("Sales Report").unwrap();
        let path = ChartResourcePath::from_name(&name);

        assert_eq!(path.as_str(), "charts/Sales Report.yssbi-chart");
        assert_eq!(path.display_name(), &name);
        assert_eq!(
            path.relative_path(),
            Path::new("charts/Sales Report.yssbi-chart")
        );

        let serialized = serde_json::to_string(&path).unwrap();
        assert_eq!(serialized, r#""charts/Sales Report.yssbi-chart""#);
        assert_eq!(
            serde_json::from_str::<ChartResourcePath>(&serialized).unwrap(),
            path
        );
    }

    #[test]
    fn chart_path_rejects_nested_wrong_extension_and_invalid_stem() {
        assert_eq!(
            ChartResourcePath::parse("reports/Sales Report.yssbi-chart"),
            Err(ChartResourcePathError::WrongDirectory)
        );
        assert_eq!(
            ChartResourcePath::parse("charts/Regional/Sales Report.yssbi-chart"),
            Err(ChartResourcePathError::Nested)
        );
        assert_eq!(
            ChartResourcePath::parse("charts/Sales Report.json"),
            Err(ChartResourcePathError::WrongExtension)
        );
        assert_eq!(
            ChartResourcePath::parse("charts/Sales?.yssbi-chart"),
            Err(ChartResourcePathError::InvalidName(
                ResourceNameValidationError::ForbiddenCharacter('?')
            ))
        );
    }

    #[test]
    fn chart_document_has_one_current_strict_wire_contract() {
        let document = ChartDocument::new("db-1");
        assert_eq!(document.schema_version(), CURRENT_CHART_SCHEMA_VERSION);
        assert_eq!(
            serde_json::to_value(&document).unwrap(),
            json!({
                "schemaVersion": CURRENT_CHART_SCHEMA_VERSION,
                "revision": 0,
                "databaseId": "db-1",
                "chartType": "histogram",
                "encodings": {}
            })
        );

        for invalid in [
            json!({
                "schemaVersion": CURRENT_CHART_SCHEMA_VERSION + 1,
                "revision": 0,
                "databaseId": "db-1",
                "chartType": "histogram",
                "encodings": {}
            }),
            json!({
                "schemaVersion": CURRENT_CHART_SCHEMA_VERSION,
                "revision": 0,
                "databaseId": "db-1",
                "chartType": "histogram",
                "encodings": { "unknown": true }
            }),
            json!({
                "schemaVersion": CURRENT_CHART_SCHEMA_VERSION,
                "revision": 0,
                "databaseId": "db-1",
                "chartType": "histogram",
                "encodings": {},
                "name": "embedded identity"
            }),
            json!({
                "schemaVersion": CURRENT_CHART_SCHEMA_VERSION,
                "databaseId": "db-1",
                "chartType": "histogram",
                "encodings": {}
            }),
        ] {
            assert!(serde_json::from_value::<ChartDocument>(invalid).is_err());
        }
    }
}
