/**
 * 回归类报告 IPC 窄化（OLS / WLS / GLS / Prais / Logit / Probit / IV 共用）
 */

import {
  assignPresentKeys,
  isFiniteNumber,
  isNonNegativeInteger,
  isRecord,
  isString,
  optionalFiniteNumber,
  optionalString,
} from './guards';
import { parseIv2slsFirstStageResult, type Iv2slsFirstStageResult } from './iv';
import { parseCoefficientList, parseFiniteNumberArray, parseModelBasicInfo } from './parseCommon';
import type { PlotPointDTO } from '@/shared/types/dto/plotPayload';
import type { BinaryModelStatistics, DiagnosticInfo, RegressionResultData } from './regression';

const DIAGNOSTIC_OPTIONAL_KEYS = [
  'vif',
  'bp_tests',
  'ov_tests',
  'im_test',
  'normality_tests',
  'fitted_values',
  'residuals',
  'leverage',
  'residual_scatter',
  'exog',
  'timing',
  'prais_info',
  'iv2sls_first_stage_summary',
  'iv2sls_overid',
  'iv2sls_overid_dims',
  'iv2sls_hausman',
  'iv2sls_endogenous',
  'ivliml_kappa',
  'ivliml_overid',
  'classification_table',
  'exog_means',
  'panel_fe_info',
  'omit_info',
] as const satisfies readonly (keyof DiagnosticInfo)[];

function parseFiniteNumberMatrix(raw: unknown): number[][] | null {
  if (!Array.isArray(raw)) return null;
  const matrix: number[][] = [];
  for (const row of raw) {
    const parsed = parseFiniteNumberArray(row);
    if (!parsed) return null;
    matrix.push(parsed);
  }
  return matrix;
}

function parseOptionalFiniteNumberArray(raw: unknown): number[] | undefined | null {
  return raw === undefined ? undefined : parseFiniteNumberArray(raw);
}

function parseOptionalFiniteNumberMatrix(raw: unknown): number[][] | undefined | null {
  return raw === undefined ? undefined : parseFiniteNumberMatrix(raw);
}

function isSquareMatrix(matrix: number[][], size: number): boolean {
  return matrix.length === size && matrix.every(row => row.length === size);
}

function equalMatrices(left: number[][], right: number[][]): boolean {
  return left.length === right.length && left.every((row, rowIndex) =>
    row.length === right[rowIndex]?.length && row.every((value, columnIndex) => value === right[rowIndex]?.[columnIndex]),
  );
}

function parseBinaryModelStatistics(raw: unknown): BinaryModelStatistics | null {
  if (!isRecord(raw) || raw.kind !== 'binary' || (raw.link !== 'logit' && raw.link !== 'probit')) {
    return null;
  }
  const covariance = parseFiniteNumberMatrix(raw.covariance);
  const standardErrors = parseFiniteNumberArray(raw.standardErrors);
  const statisticValues = parseFiniteNumberArray(raw.statisticValues);
  const pValues = parseFiniteNumberArray(raw.pValues);
  const confidenceIntervalLower = parseFiniteNumberArray(raw.confidenceIntervalLower);
  const confidenceIntervalUpper = parseFiniteNumberArray(raw.confidenceIntervalUpper);
  const coefficientArrays = [
    standardErrors,
    statisticValues,
    pValues,
    confidenceIntervalLower,
    confidenceIntervalUpper,
  ];
  if (
    !covariance ||
    coefficientArrays.some(values => values === null) ||
    !standardErrors ||
    !isSquareMatrix(covariance, standardErrors.length) ||
    coefficientArrays.some(values => values?.length !== standardErrors.length)
  ) {
    return null;
  }
  const numericKeys = [
    'logLikelihood',
    'nullLogLikelihood',
    'pseudoR2',
    'adjustedPseudoR2',
    'lrChi2',
    'lrPValue',
    'aic',
    'bic',
    'conditionNumber',
  ] as const;
  if (
    numericKeys.some(key => !isFiniteNumber(raw[key])) ||
    !isNonNegativeInteger(raw.iterations) ||
    typeof raw.converged !== 'boolean'
  ) {
    return null;
  }

  return {
    kind: 'binary',
    link: raw.link,
    covariance,
    standardErrors,
    statisticValues: statisticValues!,
    pValues: pValues!,
    confidenceIntervalLower: confidenceIntervalLower!,
    confidenceIntervalUpper: confidenceIntervalUpper!,
    logLikelihood: raw.logLikelihood as number,
    nullLogLikelihood: raw.nullLogLikelihood as number,
    pseudoR2: raw.pseudoR2 as number,
    adjustedPseudoR2: raw.adjustedPseudoR2 as number,
    lrChi2: raw.lrChi2 as number,
    lrPValue: raw.lrPValue as number,
    aic: raw.aic as number,
    bic: raw.bic as number,
    iterations: raw.iterations,
    converged: raw.converged,
    conditionNumber: raw.conditionNumber as number,
  };
}

function parseOptionalBinaryModelStatistics(raw: unknown): BinaryModelStatistics | undefined | null {
  return raw === undefined ? undefined : parseBinaryModelStatistics(raw);
}

function parseKdePoints(raw: unknown): PlotPointDTO[] | undefined | null {
  if (raw === undefined) return undefined;
  if (!Array.isArray(raw)) return null;
  const points: PlotPointDTO[] = [];
  for (const item of raw) {
    if (!isRecord(item) || !isFiniteNumber(item.x) || !isFiniteNumber(item.y)) return null;
    points.push({ x: item.x, y: item.y });
  }
  return points;
}

function parseIvFirstStageList(raw: unknown): Iv2slsFirstStageResult[] | undefined | null {
  if (raw === undefined) return undefined;
  if (!Array.isArray(raw)) return null;
  const out: Iv2slsFirstStageResult[] = [];
  for (const item of raw) {
    const parsed = parseIv2slsFirstStageResult(item);
    if (!parsed) return null;
    out.push(parsed);
  }
  return out;
}

export function parseDiagnosticInfo(raw: unknown): DiagnosticInfo | null {
  if (!isRecord(raw) || !isFiniteNumber(raw.cond_no)) return null;
  const iv2sls_first_stage = parseIvFirstStageList(raw.iv2sls_first_stage);
  const leverage_kde = parseKdePoints(raw.leverage_kde);
  if (iv2sls_first_stage === null || leverage_kde === null) return null;
  return assignPresentKeys(
    {
      cond_no: raw.cond_no,
      iv2sls_first_stage,
      leverage_kde,
    },
    raw,
    DIAGNOSTIC_OPTIONAL_KEYS,
  );
}

export function parseRegressionResultData(raw: unknown): RegressionResultData | null {
  if (!isRecord(raw) || !isString(raw.title)) return null;
  const model_basic_info = parseModelBasicInfo(raw.model_basic_info);
  const coefficients = parseCoefficientList(raw.coefficients);
  const diagnostic_info = parseDiagnosticInfo(raw.diagnostic_info);
  const betas = parseOptionalFiniteNumberArray(raw.betas);
  const cov_beta = parseOptionalFiniteNumberMatrix(raw.cov_beta);
  const model_statistics = parseOptionalBinaryModelStatistics(raw.model_statistics);
  if (
    !model_basic_info ||
    !coefficients ||
    !diagnostic_info ||
    betas === null ||
    cov_beta === null ||
    model_statistics === null
  ) {
    return null;
  }
  if (betas && betas.length !== coefficients.length) return null;
  if (cov_beta && !isSquareMatrix(cov_beta, betas?.length ?? cov_beta.length)) return null;
  if (model_statistics && model_statistics.standardErrors.length !== coefficients.length) return null;
  if (model_statistics && cov_beta && !equalMatrices(model_statistics.covariance, cov_beta)) return null;

  return {
    title: raw.title,
    endog_name: optionalString(raw, 'endog_name'),
    model_basic_info,
    coefficients,
    diagnostic_info,
    betas,
    cov_beta,
    model_statistics,
    executionTimeMs: optionalFiniteNumber(raw, 'executionTimeMs'),
  };
}
