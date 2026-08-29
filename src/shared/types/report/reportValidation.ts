import type { ResultDescriptor } from '@/shared/types/domain/result';
import { isFiniteNumber, isRecord, isString } from './guards';
import { parseReportPayload } from './parseReportPayload';
import type { ReportPayloadKind } from './reportKinds';

export interface ReportValidationDiagnostic {
  resultId: string;
  runId: string;
  activationId: string;
  nodeId: string;
  outputPinId: string | null;
  presentation: { kind: 'report'; report: ReportPayloadKind };
  valueKind: ResultDescriptor['valueKind'];
  fieldPath: string;
  reason: string;
}

export type ReportValidationResult =
  | { ok: true; value: unknown }
  | { ok: false; diagnostic: ReportValidationDiagnostic };

interface ValidationIssue {
  fieldPath: string;
  reason: string;
}

function missing(fieldPath: string): ValidationIssue {
  return { fieldPath, reason: 'missing required field' };
}

function invalid(fieldPath: string, expected: string): ValidationIssue {
  return { fieldPath, reason: `expected ${expected}` };
}

function validateOlsCanonicalValue(raw: unknown): ValidationIssue | null {
  if (!isRecord(raw)) return invalid('$', 'object');
  if (!isString(raw.title)) return raw.title === undefined ? missing('title') : invalid('title', 'string');
  if (raw.title !== 'OLS Summary') return invalid('title', 'canonical "OLS Summary" title');
  if (!isRecord(raw.model_basic_info)) {
    return raw.model_basic_info === undefined
      ? missing('model_basic_info')
      : invalid('model_basic_info', 'object');
  }
  for (const field of ['model_type', 'method', 'covariance_type'] as const) {
    const fieldPath = 'model_basic_info.' + field;
    if (raw.model_basic_info[field] === undefined) return missing(fieldPath);
    if (!isString(raw.model_basic_info[field])) return invalid(fieldPath, 'string');
  }
  for (const field of [
    'num_observation',
    'r_squared',
    'adj_r_squared',
    'f_statistic',
    'prob_f_statistic',
    'df_model',
    'df_residual',
    'df_total',
    'ss_model',
    'ss_residual',
    'ss_total',
    'ms_model',
    'ms_residual',
    'ms_total',
  ] as const) {
    const fieldPath = 'model_basic_info.' + field;
    if (raw.model_basic_info[field] === undefined) return missing(fieldPath);
    if (!isFiniteNumber(raw.model_basic_info[field])) return invalid(fieldPath, 'finite number');
  }
  if (!Array.isArray(raw.coefficients)) {
    return raw.coefficients === undefined ? missing('coefficients') : invalid('coefficients', 'array');
  }
  for (let index = 0; index < raw.coefficients.length; index += 1) {
    const coefficient = raw.coefficients[index];
    const base = `coefficients[${index}]`;
    if (!isRecord(coefficient)) return invalid(base, 'object');
    const requiredStrings = ['variable'] as const;
    for (const field of requiredStrings) {
      if (coefficient[field] === undefined) return missing(`${base}.${field}`);
      if (!isString(coefficient[field])) return invalid(`${base}.${field}`, 'string');
    }
    const requiredNumbers = [
      'coef',
      'std_err',
      't_value',
      'p_value',
      'confidence_interval_0.025',
      'confidence_interval_0.975',
    ] as const;
    for (const field of requiredNumbers) {
      if (coefficient[field] === undefined) return missing(`${base}.${field}`);
      if (!isFiniteNumber(coefficient[field])) return invalid(`${base}.${field}`, 'finite number');
    }
    if (coefficient.is_significant === undefined) return missing(`${base}.is_significant`);
    if (typeof coefficient.is_significant !== 'boolean') {
      return invalid(`${base}.is_significant`, 'boolean');
    }
  }
  if (!isRecord(raw.diagnostic_info)) {
    return raw.diagnostic_info === undefined
      ? missing('diagnostic_info')
      : invalid('diagnostic_info', 'object');
  }
  if (raw.diagnostic_info.cond_no === undefined) return missing('diagnostic_info.cond_no');
  if (!isFiniteNumber(raw.diagnostic_info.cond_no)) {
    return invalid('diagnostic_info.cond_no', 'finite number');
  }
  return null;
}

function outputPinId(descriptor: ResultDescriptor): string | null {
  const port = descriptor.provenance.output?.port;
  if (!port) return null;
  return port.kind === 'declared' ? port.portKey : `${port.templateKey}/${port.instanceId}`;
}

export function validateReportPayload(
  descriptor: ResultDescriptor,
  report: ReportPayloadKind,
  raw: unknown,
): ReportValidationResult {
  const issue = report === 'olsSummary'
    ? validateOlsCanonicalValue(raw)
    : parseReportPayload(report, raw) === null
      ? invalid('$', `canonical ${report} report`)
      : null;
  const value = issue ? null : parseReportPayload(report, raw);
  if (!issue && value !== null) return { ok: true, value };

  const resolvedIssue = issue ?? invalid('$', `canonical ${report} report`);
  return {
    ok: false,
    diagnostic: {
      resultId: descriptor.resultId,
      runId: descriptor.provenance.runId,
      activationId: descriptor.provenance.activationId,
      nodeId: descriptor.provenance.nodeId,
      outputPinId: outputPinId(descriptor),
      presentation: { kind: 'report', report },
      valueKind: descriptor.valueKind,
      fieldPath: resolvedIssue.fieldPath,
      reason: resolvedIssue.reason,
    },
  };
}
