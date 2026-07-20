use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InferenceResult {
    pub summaries: Vec<ParameterSummary>,
    pub diagnostics: InferenceDiagnostics,
    pub artifact_manifest: ResultArtifactManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ParameterSummary {
    pub parameter: String,
    pub mean: f64,
    pub sd: f64,
    pub median: f64,
    pub q025: f64,
    pub q975: f64,
    pub rhat: Option<f64>,
    pub ess_bulk: Option<f64>,
    pub ess_tail: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InferenceDiagnostics {
    pub chains: usize,
    pub draws_per_chain: usize,
    pub warmup: usize,
    pub divergences: Option<usize>,
    pub max_treedepth_hits: Option<usize>,
    pub warnings: Vec<DiagnosticWarning>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticWarning {
    pub code: String,
    pub message: String,
    pub parameter: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResultArtifactManifest {
    pub task_id: String,
    pub artifacts: Vec<ResultArtifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResultArtifact {
    pub kind: ResultArtifactKind,
    pub format: ResultArtifactFormat,
    pub path: String,
    pub rows: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResultArtifactKind {
    Summary,
    Metadata,
    PosteriorSamples,
    PosteriorPredictive,
    Log,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResultArtifactFormat {
    Json,
    ArrowIpc,
    Text,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PosteriorSampleRow {
    pub parameter: String,
    pub chain: usize,
    pub draw: usize,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PosteriorSamplePage {
    pub rows: Vec<PosteriorSampleRow>,
    pub offset: usize,
    pub limit: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TracePoint {
    pub draw: usize,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TraceSeries {
    pub parameter: String,
    pub chain: usize,
    pub points: Vec<TracePoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TracePlotData {
    pub series: Vec<TraceSeries>,
    pub max_points_per_chain: usize,
    pub stride: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DensityPoint {
    pub x: f64,
    pub density: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DensitySeries {
    pub parameter: String,
    pub points: Vec<DensityPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DensityPlotData {
    pub series: Vec<DensitySeries>,
    pub bins: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutocorrelationPlotData {
    pub series: Vec<AutocorrelationSeries>,
    pub max_lag: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutocorrelationSeries {
    pub parameter: String,
    pub chain: usize,
    pub points: Vec<AutocorrelationPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutocorrelationPoint {
    pub lag: usize,
    pub autocorrelation: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PosteriorPredictiveRow {
    pub observation: usize,
    pub observed: f64,
    pub mean: f64,
    pub q025: f64,
    pub q975: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PosteriorPredictivePage {
    pub rows: Vec<PosteriorPredictiveRow>,
    pub offset: usize,
    pub limit: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub struct TaskProgress {
    pub stage: String,
    pub completed: Option<usize>,
    pub total: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskError {
    pub code: String,
    pub message: String,
    pub detail: Option<String>,
}
