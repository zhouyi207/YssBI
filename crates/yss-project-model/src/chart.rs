use crate::file::{FileContent, FilePath, FileState, bounded, validate_file_size};
use serde::{Deserialize, Serialize};
use yss_chart_document::ChartDocument;

pub type ChartPath = FilePath<ChartDocument>;
pub type ChartState = FileState<ChartDocument>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum ChartEdit {
    SetDocument { document: ChartDocument },
}

impl FileContent for ChartDocument {
    type Edit = ChartEdit;
    const KIND: &'static str = "chart";
    const DIRECTORY: &'static str = yss_project_layout::CHARTS_DIR;
    const EXTENSION: &'static str = yss_project_layout::CHART_EXTENSION;
    fn new(_title: &str, _next_id: &mut dyn FnMut() -> String) -> Self {
        ChartDocument::new(String::new())
    }
    fn decode(bytes: &[u8]) -> Result<Self, String> {
        validate_file_size(bytes.len())?;
        serde_json::from_slice(bytes).map_err(|error| error.to_string())
    }
    fn encode(&self) -> Result<Vec<u8>, String> {
        bounded(serde_json::to_vec_pretty(self).map_err(|error| error.to_string())?)
    }
    fn apply(&mut self, edit: ChartEdit) -> Result<(), String> {
        let ChartEdit::SetDocument { document } = edit;
        *self = document;
        Ok(())
    }
    fn duplicate(&self, _next_id: &mut dyn FnMut() -> String) -> Self {
        self.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_chart_document::{CURRENT_CHART_SCHEMA_VERSION, ChartType};
    use yss_resource_naming::ResourceName;

    #[test]
    fn chart_paths_use_the_shared_canonical_file_identity() {
        let name = ResourceName::parse("Sales Report").unwrap();
        let path = ChartPath::from_name(&name);
        assert_eq!(path.as_str(), "charts/Sales Report.yssbi-chart");
        assert_eq!(path.name(), "Sales Report");
        let serialized = serde_json::to_string(&path).unwrap();
        assert_eq!(serialized, r#""charts/Sales Report.yssbi-chart""#);
        assert_eq!(
            serde_json::from_str::<ChartPath>(&serialized).unwrap(),
            path
        );
        for invalid in [
            "reports/Sales Report.yssbi-chart",
            "charts/Regional/Sales Report.yssbi-chart",
            "charts/Sales Report.json",
            "charts/Sales?.yssbi-chart",
        ] {
            assert!(ChartPath::parse(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn chart_file_body_round_trips_the_strict_document_format() {
        let mut document = ChartDocument::new("db-1");
        document.chart_type = ChartType::Line;
        document.encodings.x = Some("month".into());
        let bytes = document.encode().unwrap();
        assert_eq!(ChartDocument::decode(&bytes).unwrap(), document);
        let mut value = serde_json::to_value(&document).unwrap();
        value["schemaVersion"] = serde_json::json!(CURRENT_CHART_SCHEMA_VERSION + 1);
        assert!(ChartDocument::decode(&serde_json::to_vec(&value).unwrap()).is_err());

        let mut edited = document.clone();
        let mut replacement = ChartDocument::new("db-2");
        replacement.chart_type = ChartType::Scatter;
        edited
            .apply(ChartEdit::SetDocument {
                document: replacement.clone(),
            })
            .unwrap();
        assert_eq!(edited, replacement);
        assert_eq!(edited.duplicate(&mut || unreachable!()), replacement);
    }
}
