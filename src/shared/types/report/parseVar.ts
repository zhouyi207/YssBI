/**
 * VAR / varsoc 报告 IPC 窄化
 */

import {
  assignPresentKeys,
  isFiniteNumber,
  isNonNegativeInteger,
  isRecord,
  isString,
  optionalNonNegativeInteger,
} from './guards';
import { parseStringArray, parseObjectArray } from './parseCommon';
import type {
  VARCoefDisplay,
  VAREquationDisplay,
  VARGrangerDisplay,
  VARLmarDisplay,
  VARStableRow,
  VARSocResultData,
  VARSummaryResultData,
  VARWleDisplay,
} from './var';

function parseVarEquation(raw: unknown): VAREquationDisplay | null {
  if (!isRecord(raw) || !isString(raw.eq_name)) return null;
  const nums = ['parms', 'rmse', 'r_sq', 'chi2', 'p_chi2'] as const;
  for (const key of nums) {
    if (!isFiniteNumber(raw[key])) return null;
  }
  return {
    eq_name: raw.eq_name,
    parms: raw.parms as number,
    rmse: raw.rmse as number,
    r_sq: raw.r_sq as number,
    chi2: raw.chi2 as number,
    p_chi2: raw.p_chi2 as number,
  };
}

function parseVarCoef(raw: unknown): VARCoefDisplay | null {
  if (!isRecord(raw) || !isString(raw.eq_name) || !isString(raw.variable)) return null;
  const nums = ['coef', 'std_err', 'z_value', 'p_value', 'ci_lower', 'ci_upper'] as const;
  for (const key of nums) {
    if (!isFiniteNumber(raw[key])) return null;
  }
  return {
    eq_name: raw.eq_name,
    variable: raw.variable,
    coef: raw.coef as number,
    std_err: raw.std_err as number,
    z_value: raw.z_value as number,
    p_value: raw.p_value as number,
    ci_lower: raw.ci_lower as number,
    ci_upper: raw.ci_upper as number,
  };
}

export function parseVarStableRow(raw: unknown): VARStableRow | null {
  if (!isRecord(raw)) return null;
  if (!isFiniteNumber(raw.re) || !isFiniteNumber(raw.im) || !isFiniteNumber(raw.modulus)) return null;
  return { re: raw.re, im: raw.im, modulus: raw.modulus };
}

export function parseVarLmar(raw: unknown): VARLmarDisplay | null {
  if (!isRecord(raw) || !isNonNegativeInteger(raw.lag)) return null;
  if (!isFiniteNumber(raw.chi2) || !isFiniteNumber(raw.df) || !isFiniteNumber(raw.p_value)) return null;
  return {
    lag: raw.lag,
    chi2: raw.chi2,
    df: raw.df,
    p_value: raw.p_value,
  };
}

function parseVarWle(raw: unknown): VARWleDisplay | null {
  if (!isRecord(raw) || !isString(raw.eq_name) || !isNonNegativeInteger(raw.lag)) return null;
  if (!isFiniteNumber(raw.chi2) || !isFiniteNumber(raw.df) || !isFiniteNumber(raw.p_value)) return null;
  return {
    eq_name: raw.eq_name,
    lag: raw.lag,
    chi2: raw.chi2,
    df: raw.df,
    p_value: raw.p_value,
  };
}

function parseVarGranger(raw: unknown): VARGrangerDisplay | null {
  if (!isRecord(raw) || !isString(raw.eq_name) || !isString(raw.excluded)) return null;
  if (!isFiniteNumber(raw.chi2) || !isFiniteNumber(raw.df) || !isFiniteNumber(raw.p_value)) return null;
  return {
    eq_name: raw.eq_name,
    excluded: raw.excluded,
    chi2: raw.chi2,
    df: raw.df,
    p_value: raw.p_value,
  };
}

export function parseVarSocResultData(raw: unknown): VARSocResultData | null {
  if (!isRecord(raw) || !isString(raw.title)) return null;
  const var_names = parseStringArray(raw.var_names);
  if (!var_names || !isNonNegativeInteger(raw.maxlag) || !isFiniteNumber(raw.num_observation)) return null;
  if (!Array.isArray(raw.rows)) return null;
  return {
    title: raw.title,
    var_names,
    maxlag: raw.maxlag,
    num_observation: raw.num_observation,
    rows: raw.rows,
  };
}

const VAR_SUMMARY_OPTIONAL_KEYS = [
  'complete_sample_rows',
  'var_max_lag',
] as const satisfies readonly (keyof VARSummaryResultData)[];

export function parseVarSummaryResultData(raw: unknown): VARSummaryResultData | null {
  if (!isRecord(raw) || !isString(raw.title)) return null;
  const var_names = parseStringArray(raw.var_names);
  const equations = parseObjectArray(raw.equations, parseVarEquation);
  const coefficients = parseObjectArray(raw.coefficients, parseVarCoef);
  if (!var_names || !equations || !coefficients) return null;
  if (!isFiniteNumber(raw.num_observation) || !isFiniteNumber(raw.log_likelihood)) return null;
  if (!isFiniteNumber(raw.aic) || !isFiniteNumber(raw.fpe) || !isFiniteNumber(raw.hqic) || !isFiniteNumber(raw.sbic)) {
    return null;
  }
  if (!isFiniteNumber(raw.det_sigma_ml) || !Array.isArray(raw.sigma)) return null;
  if (!Array.isArray(raw.oirf) || !Array.isArray(raw.fevd)) return null;

  const varstable =
    raw.varstable === undefined ? undefined : parseObjectArray(raw.varstable, parseVarStableRow) ?? undefined;
  if (raw.varstable !== undefined && varstable === undefined) return null;

  const varlmar =
    raw.varlmar === undefined ? undefined : parseObjectArray(raw.varlmar, parseVarLmar) ?? undefined;
  if (raw.varlmar !== undefined && varlmar === undefined) return null;

  const varwle =
    raw.varwle === undefined ? undefined : parseObjectArray(raw.varwle, parseVarWle) ?? undefined;
  if (raw.varwle !== undefined && varwle === undefined) return null;

  const vargranger =
    raw.vargranger === undefined
      ? undefined
      : parseObjectArray(raw.vargranger, parseVarGranger) ?? undefined;
  if (raw.vargranger !== undefined && vargranger === undefined) return null;

  return assignPresentKeys(
    {
      title: raw.title,
      var_names,
      complete_sample_rows: optionalNonNegativeInteger(raw, 'complete_sample_rows'),
      var_max_lag: optionalNonNegativeInteger(raw, 'var_max_lag'),
      num_observation: raw.num_observation,
      log_likelihood: raw.log_likelihood,
      aic: raw.aic,
      fpe: raw.fpe,
      hqic: raw.hqic,
      sbic: raw.sbic,
      det_sigma_ml: raw.det_sigma_ml,
      equations,
      coefficients,
      sigma: raw.sigma,
      oirf: raw.oirf,
      fevd: raw.fevd,
      varstable,
      varlmar,
      varwle,
      vargranger,
    },
    raw,
    VAR_SUMMARY_OPTIONAL_KEYS,
  );
}
