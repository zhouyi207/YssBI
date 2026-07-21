export interface ParameterSummaryDTO {
  parameter: string;
  mean: number;
  sd: number;
  median: number;
  q025: number;
  q975: number;
  rhat?: number;
  essBulk?: number;
  essTail?: number;
}

export interface DiagnosticWarningDTO {
  code: string;
  message: string;
  parameter?: string;
}

export interface InferenceDiagnosticsDTO {
  chains: number;
  drawsPerChain: number;
  warmup: number;
  divergences?: number;
  maxTreedepthHits?: number | null;
  warnings: DiagnosticWarningDTO[];
}



export type ResultArtifactKindDTO = 'summary' | 'metadata' | 'posterior_samples' | 'posterior_predictive' | 'log';
export type ResultArtifactFormatDTO = 'json' | 'arrow_ipc' | 'text';

export interface ResultArtifactDTO {
  kind: ResultArtifactKindDTO;
  format: ResultArtifactFormatDTO;
  path: string;
  rows?: number | null;
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

export interface PosteriorSampleRowDTO {
  parameter: string;
  chain: number;
  draw: number;
  value: number;
}

export interface PosteriorSamplePageDTO {
  rows: PosteriorSampleRowDTO[];
  offset: number;
  limit: number;
  total: number;
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

export interface PosteriorPredictiveRowDTO {
  observation: number;
  observed: number;
  mean: number;
  q025: number;
  q975: number;
}

export interface PosteriorPredictivePageDTO {
  rows: PosteriorPredictiveRowDTO[];
  offset: number;
  limit: number;
  total: number;
}

export interface TaskProgressDTO {
  stage: string;
  completed?: number;
  total?: number;
}

export interface TaskErrorDTO {
  code: string;
  message: string;
  detail?: string;
}

export interface BayesInferenceTaskDTO {
  taskId: string;
  status: 'queued' | 'running' | 'cancelling' | 'cancelled' | 'completed' | 'failed';
  progress?: TaskProgressDTO;
  error?: TaskErrorDTO;
}
