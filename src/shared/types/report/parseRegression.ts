/**
 * 回归类报告 IPC 窄化（OLS / WLS / GLS / Prais / Logit / Probit / IV 共用）
 */

import { assignPresentKeys, isFiniteNumber, isRecord, isString, optionalFiniteNumber, optionalString } from './guards';
import { parseIv2slsFirstStageResult, type Iv2slsFirstStageResult } from './iv';
import { parseCoefficientList, parseModelBasicInfo } from './parseCommon';
import type { PlotPointDTO } from '@/shared/types/dto/plotPayload';
import type { DiagnosticInfo, RegressionResultData } from './regression';

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

const REGRESSION_OPTIONAL_KEYS = ['betas', 'cov_beta'] as const satisfies readonly (keyof RegressionResultData)[];

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
  if (!model_basic_info || !coefficients || !diagnostic_info) return null;
  return assignPresentKeys(
    {
      title: raw.title,
      endog_name: optionalString(raw, 'endog_name'),
      model_basic_info,
      coefficients,
      diagnostic_info,
      executionTimeMs: optionalFiniteNumber(raw, 'executionTimeMs'),
    },
    raw,
    REGRESSION_OPTIONAL_KEYS,
  );
}
