import type {
  BayesInferenceTaskDTO,
  DiagnosticMetricDTO,
  DiagnosticWarningDTO,
  InferenceResultDTO,
  ParameterSummaryDTO,
  ResultArtifactDTO,
  TaskErrorDTO,
  TaskErrorDetailsDTO,
  TaskProgressDTO,
} from './result';
import type { ValidationIssueDTO, ValidationReportDTO } from './validation';

const TASK_STATUSES = new Set<BayesInferenceTaskDTO['status']>([
  'queued',
  'running',
  'cancelling',
  'cancelled',
  'completed',
  'failed',
]);
const DIAGNOSTIC_METRICS = new Set<DiagnosticMetricDTO>(['rhat', 'ess_bulk', 'ess_tail']);
const ARTIFACT_KINDS = new Set<ResultArtifactDTO['kind']>([
  'summary',
  'metadata',
  'posterior_samples',
  'posterior_predictive',
  'log',
]);
const ARTIFACT_FORMATS = new Set<ResultArtifactDTO['format']>(['json', 'arrow_ipc', 'text']);
const TASK_DETAIL_KEYS = ['column', 'row', 'parameter', 'path'] as const;
const LOWER_SNAKE_CODE = /^[a-z][a-z0-9]*(?:_[a-z0-9]+)*$/;

export function parseBayesInferenceTaskDTO(value: unknown): BayesInferenceTaskDTO {
  if (!isRecord(value)
    || !hasExactKeys(value, ['taskId', 'status', 'progress', 'error'])
    || !isNonEmptyString(value.taskId)
    || typeof value.status !== 'string'
    || !TASK_STATUSES.has(value.status as BayesInferenceTaskDTO['status'])) {
    return fail('Invalid Bayes inference task response');
  }

  const progress = value.progress === null ? null : parseTaskProgress(value.progress);
  const error = value.error === null ? null : parseTaskError(value.error);
  if ((value.status === 'failed') !== (error !== null)) {
    return fail('Invalid Bayes inference task response');
  }

  return {
    taskId: value.taskId,
    status: value.status as BayesInferenceTaskDTO['status'],
    progress,
    error,
  };
}

export function parseInferenceResultDTO(value: unknown): InferenceResultDTO {
  if (!isRecord(value)
    || !hasExactKeys(value, ['summaries', 'diagnostics', 'artifactManifest'])
    || !Array.isArray(value.summaries)
    || !isRecord(value.diagnostics)
    || !isRecord(value.artifactManifest)) {
    return fail('Invalid Bayes inference result response');
  }

  const diagnostics = value.diagnostics;
  if (!hasExactKeys(diagnostics, [
    'chains',
    'drawsPerChain',
    'warmup',
    'divergences',
    'maxTreedepthHits',
    'warnings',
  ])
    || !isNonNegativeInteger(diagnostics.chains)
    || !isNonNegativeInteger(diagnostics.drawsPerChain)
    || !isNonNegativeInteger(diagnostics.warmup)
    || !isNullableNonNegativeInteger(diagnostics.divergences)
    || !isNullableNonNegativeInteger(diagnostics.maxTreedepthHits)
    || !Array.isArray(diagnostics.warnings)) {
    return fail('Invalid Bayes inference result response');
  }

  const manifest = value.artifactManifest;
  if (!hasExactKeys(manifest, ['taskId', 'artifacts'])
    || !isNonEmptyString(manifest.taskId)
    || !Array.isArray(manifest.artifacts)) {
    return fail('Invalid Bayes inference result response');
  }

  return {
    summaries: value.summaries.map(parseParameterSummary),
    diagnostics: {
      chains: diagnostics.chains,
      drawsPerChain: diagnostics.drawsPerChain,
      warmup: diagnostics.warmup,
      divergences: diagnostics.divergences,
      maxTreedepthHits: diagnostics.maxTreedepthHits,
      warnings: diagnostics.warnings.map(parseDiagnosticWarning),
    },
    artifactManifest: {
      taskId: manifest.taskId,
      artifacts: manifest.artifacts.map(parseResultArtifact),
    },
  };
}

export function parseValidationReportDTO(value: unknown): ValidationReportDTO {
  if (!isRecord(value)
    || !hasExactKeys(value, ['ok', 'errors', 'warnings'])
    || typeof value.ok !== 'boolean'
    || !Array.isArray(value.errors)
    || !Array.isArray(value.warnings)) {
    return fail('Invalid Bayes validation response');
  }

  const errors = value.errors.map(parseValidationIssue);
  const warnings = value.warnings.map(parseValidationIssue);
  if (errors.some(issue => issue.severity !== 'error')
    || warnings.some(issue => issue.severity !== 'warning')
    || value.ok !== (errors.length === 0)) {
    return fail('Invalid Bayes validation response');
  }
  return { ok: value.ok, errors, warnings };
}

function parseTaskProgress(value: unknown): TaskProgressDTO {
  if (!isRecord(value)
    || !hasExactKeys(value, ['stage', 'completed', 'total'])
    || !isNonEmptyString(value.stage)
    || !isNullableNonNegativeInteger(value.completed)
    || !isNullableNonNegativeInteger(value.total)) {
    return fail('Invalid Bayes inference task response');
  }
  return { stage: value.stage, completed: value.completed, total: value.total };
}

function parseTaskError(value: unknown): TaskErrorDTO {
  if (!isRecord(value)
    || !hasExactKeys(value, ['code', 'details', 'incidentId'])
    || !isStableCode(value.code)
    || !(value.incidentId === null || isNonEmptyString(value.incidentId))) {
    return fail('Invalid Bayes inference task response');
  }
  return {
    code: value.code,
    details: value.details === null ? null : parseTaskErrorDetailsDTO(value.details),
    incidentId: value.incidentId,
  };
}

export function parseTaskErrorDetailsDTO(value: unknown): TaskErrorDetailsDTO {
  if (!isRecord(value)
    || Object.keys(value).some(key => !TASK_DETAIL_KEYS.includes(key as typeof TASK_DETAIL_KEYS[number]))) {
    return fail('Invalid Bayes inference task response');
  }
  if (value.column !== undefined && !isNonEmptyString(value.column)) return fail('Invalid Bayes inference task response');
  if (value.row !== undefined && !isNonNegativeInteger(value.row)) return fail('Invalid Bayes inference task response');
  if (value.parameter !== undefined && !isNonEmptyString(value.parameter)) return fail('Invalid Bayes inference task response');
  if (value.path !== undefined && !isNonEmptyString(value.path)) return fail('Invalid Bayes inference task response');
  return {
    ...(value.column === undefined ? {} : { column: value.column }),
    ...(value.row === undefined ? {} : { row: value.row }),
    ...(value.parameter === undefined ? {} : { parameter: value.parameter }),
    ...(value.path === undefined ? {} : { path: value.path }),
  };
}

function parseParameterSummary(value: unknown): ParameterSummaryDTO {
  if (!isRecord(value)
    || !hasExactKeys(value, [
      'parameter', 'mean', 'sd', 'median', 'q025', 'q975', 'rhat', 'essBulk', 'essTail',
    ])
    || !isNonEmptyString(value.parameter)
    || !isFiniteNumber(value.mean)
    || !isFiniteNumber(value.sd)
    || !isFiniteNumber(value.median)
    || !isFiniteNumber(value.q025)
    || !isFiniteNumber(value.q975)
    || !isNullableFiniteNumber(value.rhat)
    || !isNullableFiniteNumber(value.essBulk)
    || !isNullableFiniteNumber(value.essTail)) {
    return fail('Invalid Bayes inference result response');
  }
  return {
    parameter: value.parameter,
    mean: value.mean,
    sd: value.sd,
    median: value.median,
    q025: value.q025,
    q975: value.q975,
    rhat: value.rhat,
    essBulk: value.essBulk,
    essTail: value.essTail,
  };
}

function parseDiagnosticWarning(value: unknown): DiagnosticWarningDTO {
  if (!isRecord(value)
    || !hasExactKeys(value, ['code', 'metric', 'value', 'threshold', 'parameter'])
    || !isStableCode(value.code)
    || typeof value.metric !== 'string'
    || !DIAGNOSTIC_METRICS.has(value.metric as DiagnosticMetricDTO)
    || !isFiniteNumber(value.value)
    || !isFiniteNumber(value.threshold)
    || !isNonEmptyString(value.parameter)) {
    return fail('Invalid Bayes inference result response');
  }
  return {
    code: value.code,
    metric: value.metric as DiagnosticMetricDTO,
    value: value.value,
    threshold: value.threshold,
    parameter: value.parameter,
  };
}

function parseResultArtifact(value: unknown): ResultArtifactDTO {
  if (!isRecord(value)
    || !hasExactKeys(value, ['kind', 'format', 'path', 'rows'])
    || typeof value.kind !== 'string'
    || !ARTIFACT_KINDS.has(value.kind as ResultArtifactDTO['kind'])
    || typeof value.format !== 'string'
    || !ARTIFACT_FORMATS.has(value.format as ResultArtifactDTO['format'])
    || !isNonEmptyString(value.path)
    || !isNullableNonNegativeInteger(value.rows)) {
    return fail('Invalid Bayes inference result response');
  }
  return {
    kind: value.kind as ResultArtifactDTO['kind'],
    format: value.format as ResultArtifactDTO['format'],
    path: value.path,
    rows: value.rows,
  };
}

function parseValidationIssue(value: unknown): ValidationIssueDTO {
  if (!isRecord(value)
    || !hasExactKeys(value, ['code', 'severity', 'path'])
    || !isStableCode(value.code)
    || (value.severity !== 'error' && value.severity !== 'warning')
    || !isNonEmptyString(value.path)) {
    return fail('Invalid Bayes validation response');
  }
  return { code: value.code, severity: value.severity, path: value.path };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0;
}

function isStableCode(value: unknown): value is string {
  return typeof value === 'string' && LOWER_SNAKE_CODE.test(value);
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}

function isNullableFiniteNumber(value: unknown): value is number | null {
  return value === null || isFiniteNumber(value);
}

function isNonNegativeInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0;
}

function isNullableNonNegativeInteger(value: unknown): value is number | null {
  return value === null || isNonNegativeInteger(value);
}

function fail(message: string): never {
  throw new Error(message);
}
