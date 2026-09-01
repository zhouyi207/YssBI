export interface ParameterSummaryDTO {
  parameter: string;
  mean: number;
  sd: number;
  median: number;
  q025: number;
  q975: number;
  rhat: number | null;
  essBulk: number | null;
  essTail: number | null;
}

export type DiagnosticMetricDTO = "rhat" | "ess_bulk" | "ess_tail";

export interface DiagnosticWarningDTO {
  code: string;
  metric: DiagnosticMetricDTO;
  value: number;
  threshold: number;
  parameter: string;
}

export interface InferenceDiagnosticsDTO {
  chains: number;
  drawsPerChain: number;
  warmup: number;
  divergences: number | null;
  maxTreedepthHits: number | null;
  warnings: DiagnosticWarningDTO[];
}

export type ResultArtifactKindDTO =
  | "summary"
  | "metadata"
  | "posterior_samples"
  | "posterior_predictive"
  | "log";
export type ResultArtifactFormatDTO = "json" | "arrow_ipc" | "text";

export interface ResultArtifactDTO {
  kind: ResultArtifactKindDTO;
  format: ResultArtifactFormatDTO;
  path: string;
  rows: number | null;
}

export interface ResultArtifactManifestDTO {
  taskId: string;
  artifacts: ResultArtifactDTO[];
}

export interface InferenceResultDTO {
  summaries: ParameterSummaryDTO[];
  diagnostics: InferenceDiagnosticsDTO;
  artifactManifest: ResultArtifactManifestDTO;
}

export interface TracePointDTO {
  draw: number;
  value: number;
}

export interface TraceSeriesDTO {
  parameter: string;
  chain: number;
  points: TracePointDTO[];
}

export interface TracePlotDataDTO {
  series: TraceSeriesDTO[];
  maxPointsPerChain: number;
  stride: number;
}

export interface DensityPointDTO {
  x: number;
  density: number;
}

export interface DensitySeriesDTO {
  parameter: string;
  chain: number | null;
  points: DensityPointDTO[];
}

export interface DensityPlotDataDTO {
  series: DensitySeriesDTO[];
  gridPoints: number;
}

export interface AutocorrelationPointDTO {
  lag: number;
  autocorrelation: number;
}

export interface AutocorrelationSeriesDTO {
  parameter: string;
  chain: number;
  points: AutocorrelationPointDTO[];
}

export interface AutocorrelationPlotDataDTO {
  series: AutocorrelationSeriesDTO[];
  maxLag: number;
}

export interface PosteriorPredictiveSummaryDTO {
  observed: number;
  mean: number;
  q025: number;
  q975: number;
}

export interface PosteriorPredictiveRowDTO {
  observation: number;
  model: PosteriorPredictiveSummaryDTO;
  original: PosteriorPredictiveSummaryDTO;
}

export interface PosteriorPredictivePageDTO {
  rows: PosteriorPredictiveRowDTO[];
  responseTransform: "identity" | "ln";
  offset: number;
  limit: number;
  total: number;
}

export interface TaskProgressDTO {
  stage: string;
  completed: number | null;
  total: number | null;
}

export interface TaskErrorDetailsDTO {
  column?: string;
  row?: number;
  parameter?: string;
  path?: string;
}

export interface TaskErrorDTO {
  code: string;
  details: TaskErrorDetailsDTO | null;
  incidentId: string | null;
}

export interface BayesInferenceTaskDTO {
  taskId: string;
  status: "queued" | "running" | "cancelling" | "cancelled" | "completed" | "failed";
  progress: TaskProgressDTO | null;
  error: TaskErrorDTO | null;
}
