/**
 * IV / 2SLS / LIML 报告 DTO（对齐 Rust `info_nodes.rs`）
 */

import { isFiniteNumber, isRecord, isString, isStringArray } from './guards';
import type { Coefficient } from './regression';
import { parseCoefficientList } from './parseCommon';

export interface IvLimlOveridTest {
  anderson_rubin_stat: number;
  anderson_rubin_p_value: number;
  basmann_stat: number;
  basmann_p_value: number;
  df: number;
  df_denom: number;
}

export interface Iv2slsHausmanTest {
  stat: number;
  p_value: number;
  df: number;
}

export interface Iv2slsEndogenousTest {
  durbin_stat: number;
  durbin_p_value: number;
  wu_stat: number;
  wu_p_value: number;
  df: number;
  wu_df_denom: number;
}

export interface Iv2slsStockYogoBiasRow {
  pct_5: number;
  pct_10: number;
  pct_20: number;
  pct_30: number;
}

export interface Iv2slsStockYogoSizeRow {
  pct_10: number;
  pct_15: number;
  pct_20: number;
  pct_25: number;
}

export interface Iv2slsStockYogoCv {
  bias: Iv2slsStockYogoBiasRow | null;
  size: Iv2slsStockYogoSizeRow;
}

export interface Iv2slsOveridTest {
  test_type: 'sargan_basmann' | 'wooldridge';
  sargan_stat?: number;
  sargan_p_value?: number;
  basmann_stat?: number;
  basmann_p_value?: number;
  wooldridge_stat?: number;
  wooldridge_p_value?: number;
  df: number;
}

export interface Iv2slsFirstStageSummary {
  k_included_instruments: number;
  k_excluded_instruments: number;
  k_endogenous_regressors: number;
  r2?: number;
  r2_adjusted?: number;
  partial_r2?: number;
  f_stat?: number;
  f_p_value?: number;
  f_df1?: number;
  f_df2?: number;
  shea_partial_r2: number[];
  shea_adj_partial_r2: number[];
  min_eigenvalue: number;
  min_eigenvalue_cv?: Iv2slsStockYogoCv;
  min_eigenvalue_cv_note?: string;
}

export interface Iv2slsFirstStageResult {
  endog_name: string;
  var_names: string[];
  coefficients: Coefficient[];
  r_squared: number;
  adj_r_squared: number;
}

/** 窄化 IV 第一阶段单方程结果（Rust `Iv2slsFirstStageResult`） */
export function parseIv2slsFirstStageResult(raw: unknown): Iv2slsFirstStageResult | null {
  if (!isRecord(raw) || !isString(raw.endog_name) || !isStringArray(raw.var_names)) return null;
  if (!isFiniteNumber(raw.r_squared) || !isFiniteNumber(raw.adj_r_squared)) return null;
  const coefficients = parseCoefficientList(raw.coefficients);
  if (!coefficients) return null;
  return {
    endog_name: raw.endog_name,
    var_names: raw.var_names,
    coefficients,
    r_squared: raw.r_squared,
    adj_r_squared: raw.adj_r_squared,
  };
}
