//! Bayesian task, artifact, plot, and paging result projections.

use serde::{Deserialize, Serialize};

use super::diagnostics::{InferenceDiagnostics, ParameterSummary};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InferenceResult {
    summaries: Vec<ParameterSummary>,
    diagnostics: InferenceDiagnostics,
    artifact_manifest: ResultArtifactManifest,
}

impl InferenceResult {
    pub fn new(
        summaries: Vec<ParameterSummary>,
        diagnostics: InferenceDiagnostics,
        artifact_manifest: ResultArtifactManifest,
    ) -> Self {
        Self {
            summaries,
            diagnostics,
            artifact_manifest,
        }
    }

    pub fn summaries(&self) -> &[ParameterSummary] {
        &self.summaries
    }

    pub fn diagnostics(&self) -> &InferenceDiagnostics {
        &self.diagnostics
    }

    pub fn artifact_manifest(&self) -> &ResultArtifactManifest {
        &self.artifact_manifest
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResultArtifactManifest {
    task_id: String,
    artifacts: Vec<ResultArtifact>,
}

impl ResultArtifactManifest {
    pub fn from_worker(task_id: impl Into<String>, artifacts: Vec<ResultArtifact>) -> Self {
        Self {
            task_id: task_id.into(),
            artifacts,
        }
    }

    pub fn task_id(&self) -> &str {
        &self.task_id
    }

    pub fn artifacts(&self) -> &[ResultArtifact] {
        &self.artifacts
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResultArtifact {
    kind: ResultArtifactKind,
    format: ResultArtifactFormat,
    path: String,
    rows: Option<usize>,
}

impl ResultArtifact {
    pub fn from_worker(
        kind: ResultArtifactKind,
        format: ResultArtifactFormat,
        path: impl Into<String>,
        rows: Option<usize>,
    ) -> Self {
        Self {
            kind,
            format,
            path: path.into(),
            rows,
        }
    }

    pub fn kind(&self) -> ResultArtifactKind {
        self.kind
    }

    pub fn format(&self) -> ResultArtifactFormat {
        self.format
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn rows(&self) -> Option<usize> {
        self.rows
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResultArtifactKind {
    Summary,
    Metadata,
    PosteriorSamples,
    PosteriorPredictive,
    Log,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResultArtifactFormat {
    Json,
    ArrowIpc,
    Text,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PosteriorSampleRow {
    pub parameter: String,
    pub chain: usize,
    pub draw: usize,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PosteriorSamplePage {
    pub rows: Vec<PosteriorSampleRow>,
    pub offset: usize,
    pub limit: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TracePoint {
    pub draw: usize,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TraceSeries {
    pub parameter: String,
    pub chain: usize,
    pub points: Vec<TracePoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TracePlotData {
    pub series: Vec<TraceSeries>,
    pub max_points_per_chain: usize,
    pub stride: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DensityPoint {
    pub x: f64,
    pub density: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DensitySeries {
    pub parameter: String,
    pub chain: Option<usize>,
    pub points: Vec<DensityPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DensityPlotData {
    pub series: Vec<DensitySeries>,
    pub grid_points: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutocorrelationPlotData {
    pub series: Vec<AutocorrelationSeries>,
    pub max_lag: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutocorrelationSeries {
    pub parameter: String,
    pub chain: usize,
    pub points: Vec<AutocorrelationPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutocorrelationPoint {
    pub lag: usize,
    pub autocorrelation: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PosteriorPredictiveSummary {
    pub observed: f64,
    pub mean: f64,
    pub q025: f64,
    pub q975: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PosteriorPredictiveRow {
    pub observation: usize,
    pub model: PosteriorPredictiveSummary,
    pub original: PosteriorPredictiveSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PosteriorPredictivePage {
    pub rows: Vec<PosteriorPredictiveRow>,
    pub response_transform: String,
    pub offset: usize,
    pub limit: usize,
    pub total: usize,
}

#[cfg(test)]
mod tests {
    use super::{DensityPlotData, TaskError};
    use crate::diagnostics::DiagnosticWarning;

    #[test]
    fn task_errors_and_diagnostic_warnings_use_safe_structured_wire_shapes() {
        let task_error: TaskError = serde_json::from_value(serde_json::json!({
            "code": "julia_bayes_invalid_data",
            "details": {
                "column": "predictor_x",
                "row": 7,
                "parameter": "beta",
                "path": "parameters.beta"
            },
            "incidentId": null
        }))
        .expect("deserialize safe task error");
        assert_eq!(
            serde_json::to_value(task_error).expect("serialize safe task error"),
            serde_json::json!({
                "code": "julia_bayes_invalid_data",
                "details": {
                    "column": "predictor_x",
                    "row": 7,
                    "parameter": "beta",
                    "path": "parameters.beta"
                },
                "incidentId": null
            })
        );
        assert!(
            serde_json::from_value::<TaskError>(serde_json::json!({
                "code": "julia_bayes_sampling_failed",
                "details": null,
                "incidentId": "incident-42",
                "message": "private backend message"
            }))
            .is_err(),
            "task error must reject legacy display fields"
        );
        assert!(
            serde_json::from_value::<TaskError>(serde_json::json!({
                "code": "julia_bayes_invalid_data",
                "details": { "column": "x", "detail": "private backend detail" },
                "incidentId": null
            }))
            .is_err(),
            "task error details must reject backend prose"
        );

        let warning: DiagnosticWarning = serde_json::from_value(serde_json::json!({
            "code": "ess_too_low",
            "metric": "ess_tail",
            "value": 42.5,
            "threshold": 100.0,
            "parameter": "beta"
        }))
        .expect("deserialize structured diagnostic warning");
        assert_eq!(
            serde_json::to_value(warning).expect("serialize structured diagnostic warning"),
            serde_json::json!({
                "code": "ess_too_low",
                "metric": "ess_tail",
                "value": 42.5,
                "threshold": 100.0,
                "parameter": "beta"
            })
        );
        assert!(
            serde_json::from_value::<DiagnosticWarning>(serde_json::json!({
                "code": "ess_too_low",
                "metric": "ess_tail",
                "value": 42.5,
                "threshold": 100.0,
                "parameter": "beta",
                "message": "increase samples"
            }))
            .is_err(),
            "diagnostic warning must reject legacy display fields"
        );
    }

    #[test]
    fn density_plot_data_uses_grid_points_json_contract() {
        let value = serde_json::to_value(DensityPlotData {
            series: Vec::new(),
            grid_points: 64,
        })
        .expect("serialize density plot data");

        assert_eq!(value["gridPoints"], 64);
        assert!(value.get("bins").is_none());
        assert!(
            serde_json::from_value::<DensityPlotData>(serde_json::json!({
                "series": [],
                "gridPoints": 64,
                "bins": 64
            }))
            .is_err()
        );
    }

    #[test]
    fn result_artifacts_reject_unknown_fields_and_unsupported_formats() {
        let artifact = serde_json::json!({
            "kind": "posterior_samples",
            "format": "arrow_ipc",
            "path": "results/samples.arrow",
            "rows": 10,
            "owner": "legacy"
        });
        assert!(serde_json::from_value::<super::ResultArtifact>(artifact).is_err());

        let unsupported = serde_json::json!({
            "kind": "posterior_samples",
            "format": "png",
            "path": "results/plot.png",
            "rows": null
        });
        assert!(serde_json::from_value::<super::ResultArtifact>(unsupported).is_err());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BayesInferenceTask {
    pub task_id: String,
    pub status: TaskStatus,
    pub progress: Option<TaskProgress>,
    pub error: Option<TaskError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Running,
    Cancelling,
    Cancelled,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskProgress {
    pub stage: String,
    pub completed: Option<usize>,
    pub total: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskError {
    pub code: String,
    pub details: Option<TaskErrorDetails>,
    pub incident_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskErrorDetails {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl TaskErrorDetails {
    pub fn is_empty(&self) -> bool {
        self.column.is_none()
            && self.row.is_none()
            && self.parameter.is_none()
            && self.path.is_none()
    }
}
