use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CURRENT_CHART_SCHEMA_VERSION: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChartType {
    Histogram,
    Scatter,
    Line,
}

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
    pub database_id: String,
    pub chart_type: ChartType,
    pub encodings: ChartEncodings,
}

impl ChartDocument {
    pub fn new(database_id: impl Into<String>) -> Self {
        Self {
            schema_version: CURRENT_CHART_SCHEMA_VERSION,
            database_id: database_id.into(),
            chart_type: ChartType::Histogram,
            encodings: ChartEncodings { x: None, y: None },
        }
    }

    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{CURRENT_CHART_SCHEMA_VERSION, ChartDocument, ChartType};

    #[test]
    fn chart_document_has_one_current_strict_wire_contract() {
        let document = ChartDocument::new("db-1");
        assert_eq!(document.schema_version(), CURRENT_CHART_SCHEMA_VERSION);
        assert_eq!(
            serde_json::to_value(&document).unwrap(),
            json!({
                "schemaVersion": CURRENT_CHART_SCHEMA_VERSION,
                "databaseId": "db-1",
                "chartType": "histogram",
                "encodings": {}
            })
        );

        for (chart_type, wire) in [
            (ChartType::Histogram, "histogram"),
            (ChartType::Scatter, "scatter"),
            (ChartType::Line, "line"),
        ] {
            let mut document = document.clone();
            document.chart_type = chart_type;
            let value = serde_json::to_value(&document).unwrap();
            assert_eq!(value["chartType"], wire);
            assert_eq!(
                serde_json::from_value::<ChartDocument>(value).unwrap(),
                document
            );
        }

        for invalid in [
            json!({
                "schemaVersion": CURRENT_CHART_SCHEMA_VERSION,
                "databaseId": "db-1",
                "chartType": "unsupported",
                "encodings": {}
            }),
            json!({
                "schemaVersion": CURRENT_CHART_SCHEMA_VERSION + 1,
                "databaseId": "db-1",
                "chartType": "histogram",
                "encodings": {}
            }),
            json!({
                "schemaVersion": CURRENT_CHART_SCHEMA_VERSION,
                "databaseId": "db-1",
                "chartType": "histogram",
                "encodings": { "unknown": true }
            }),
            json!({
                "schemaVersion": CURRENT_CHART_SCHEMA_VERSION,
                "databaseId": "db-1",
                "chartType": "histogram",
                "encodings": {},
                "name": "embedded identity"
            }),
            json!({
                "schemaVersion": CURRENT_CHART_SCHEMA_VERSION,
                "revision": 0,
                "databaseId": "db-1",
                "chartType": "histogram",
                "encodings": {}
            }),
        ] {
            assert!(serde_json::from_value::<ChartDocument>(invalid).is_err());
        }
    }
}
