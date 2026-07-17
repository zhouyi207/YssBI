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
  maxTreedepthHits?: number;
  warnings: DiagnosticWarningDTO[];
}

export interface InferenceResultRefDTO {
  taskId: string;
  summaryPath?: string;
  samplesPath?: string;
  metadataPath?: string;
}

export interface InferenceResultDTO {
  summaries: ParameterSummaryDTO[];
  diagnostics: InferenceDiagnosticsDTO;
  samples?: InferenceResultRefDTO;
  logPath?: string;
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
  result?: InferenceResultRefDTO;
  error?: TaskErrorDTO;
}
