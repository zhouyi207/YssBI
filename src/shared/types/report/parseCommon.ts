/**
 * 报告 JSON 共用窄化（系数、模型摘要等）
 */

import {
  assignPresentKeys,
  isFiniteNumber,
  isNonNegativeInteger,
  isRecord,
  isString,
} from './guards';
import type { Coefficient, ModelBasicInfo } from './regression';

export function parseCoefficient(raw: unknown): Coefficient | null {
  if (!isRecord(raw) || !isString(raw.variable) || !isFiniteNumber(raw.coef)) return null;
  if (typeof raw.is_significant !== 'boolean') return null;
  return {
    variable: raw.variable,
    category: typeof raw.category === 'string' ? raw.category : undefined,
    coef: raw.coef,
    std_err: isFiniteNumber(raw.std_err) ? raw.std_err : undefined,
    t_value: isFiniteNumber(raw.t_value) ? raw.t_value : undefined,
    p_value: isFiniteNumber(raw.p_value) ? raw.p_value : undefined,
    'confidence_interval_0.025': isFiniteNumber(raw['confidence_interval_0.025'])
      ? raw['confidence_interval_0.025']
      : undefined,
    'confidence_interval_0.975': isFiniteNumber(raw['confidence_interval_0.975'])
      ? raw['confidence_interval_0.975']
      : undefined,
    is_significant: raw.is_significant,
  };
}

export function parseCoefficientList(raw: unknown): Coefficient[] | null {
  if (!Array.isArray(raw)) return null;
  const out: Coefficient[] = [];
  for (const item of raw) {
    const coef = parseCoefficient(item);
    if (!coef) return null;
    out.push(coef);
  }
  return out;
}

const MODEL_BASIC_OPTIONAL_KEYS = [
  'wald_chi2',
  'prob_wald_chi2',
  'log_likelihood',
  'lr_chi2',
  'prob_lr_chi2',
  'chibar2',
  'prob_chibar2',
  'mle_iter_log_lik_const',
  'mle_iter_log_lik',
  'aic',
  'bic',
] as const satisfies readonly (keyof ModelBasicInfo)[];

export function parseModelBasicInfo(raw: unknown): ModelBasicInfo | null {
  if (!isRecord(raw)) return null;
  const requiredNumbers = [
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
  ] as const;
  for (const key of requiredNumbers) {
    if (!isFiniteNumber(raw[key])) return null;
  }
  if (!isString(raw.model_type) || !isString(raw.method) || !isString(raw.covariance_type)) {
    return null;
  }
  return assignPresentKeys(
    {
      model_type: raw.model_type,
      method: raw.method,
      num_observation: raw.num_observation as number,
      r_squared: raw.r_squared as number,
      adj_r_squared: raw.adj_r_squared as number,
      f_statistic: raw.f_statistic as number,
      prob_f_statistic: raw.prob_f_statistic as number,
      df_model: raw.df_model as number,
      df_residual: raw.df_residual as number,
      df_total: raw.df_total as number,
      ss_model: raw.ss_model as number,
      ss_residual: raw.ss_residual as number,
      ss_total: raw.ss_total as number,
      ms_model: raw.ms_model as number,
      ms_residual: raw.ms_residual as number,
      ms_total: raw.ms_total as number,
      covariance_type: raw.covariance_type,
    },
    raw,
    MODEL_BASIC_OPTIONAL_KEYS,
  );
}

export function parseFiniteNumberArray(raw: unknown): number[] | null {
  if (!Array.isArray(raw)) return null;
  if (!raw.every(isFiniteNumber)) return null;
  return raw;
}

export function parseStringArray(raw: unknown): string[] | null {
  if (!Array.isArray(raw)) return null;
  if (!raw.every(isString)) return null;
  return raw;
}

export function parseNonNegativeIntArray(raw: unknown): number[] | null {
  if (!Array.isArray(raw)) return null;
  if (!raw.every(isNonNegativeInteger)) return null;
  return raw;
}

export function parseObjectArray<T>(
  raw: unknown,
  parseItem: (item: unknown) => T | null,
): T[] | null {
  if (!Array.isArray(raw)) return null;
  const out: T[] = [];
  for (const item of raw) {
    const parsed = parseItem(item);
    if (!parsed) return null;
    out.push(parsed);
  }
  return out;
}
