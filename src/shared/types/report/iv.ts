import {
  numberField,
  stringField,
  literalField,
  optionalField,
  nullableField,
  arrayField,
  objectField,
} from "./fields";
import { coefficientField } from "./parseCommon";

/**
 * IV / 2SLS / LIML 报告 DTO（对齐 Rust `info_nodes.rs`）
 */

import type { Coefficient } from "./regression";

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
  test_type: "sargan_basmann" | "wooldridge";
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

export const ivLimlOveridTestField = objectField<IvLimlOveridTest>({
  anderson_rubin_stat: numberField,
  anderson_rubin_p_value: numberField,
  basmann_stat: numberField,
  basmann_p_value: numberField,
  df: numberField,
  df_denom: numberField,
});

export const iv2slsHausmanTestField = objectField<Iv2slsHausmanTest>({
  stat: numberField,
  p_value: numberField,
  df: numberField,
});

export const iv2slsEndogenousTestField = objectField<Iv2slsEndogenousTest>({
  durbin_stat: numberField,
  durbin_p_value: numberField,
  wu_stat: numberField,
  wu_p_value: numberField,
  df: numberField,
  wu_df_denom: numberField,
});

const iv2slsStockYogoBiasRowField = objectField<Iv2slsStockYogoBiasRow>({
  pct_5: numberField,
  pct_10: numberField,
  pct_20: numberField,
  pct_30: numberField,
});

const iv2slsStockYogoSizeRowField = objectField<Iv2slsStockYogoSizeRow>({
  pct_10: numberField,
  pct_15: numberField,
  pct_20: numberField,
  pct_25: numberField,
});

const iv2slsStockYogoCvField = objectField<Iv2slsStockYogoCv>({
  bias: nullableField(iv2slsStockYogoBiasRowField),
  size: iv2slsStockYogoSizeRowField,
});

export const iv2slsOveridTestField = objectField<Iv2slsOveridTest>({
  test_type: literalField("sargan_basmann", "wooldridge"),
  sargan_stat: optionalField(numberField),
  sargan_p_value: optionalField(numberField),
  basmann_stat: optionalField(numberField),
  basmann_p_value: optionalField(numberField),
  wooldridge_stat: optionalField(numberField),
  wooldridge_p_value: optionalField(numberField),
  df: numberField,
});

export const iv2slsFirstStageSummaryField = objectField<Iv2slsFirstStageSummary>({
  k_included_instruments: numberField,
  k_excluded_instruments: numberField,
  k_endogenous_regressors: numberField,
  r2: optionalField(numberField),
  r2_adjusted: optionalField(numberField),
  partial_r2: optionalField(numberField),
  f_stat: optionalField(numberField),
  f_p_value: optionalField(numberField),
  f_df1: optionalField(numberField),
  f_df2: optionalField(numberField),
  shea_partial_r2: arrayField(numberField),
  shea_adj_partial_r2: arrayField(numberField),
  min_eigenvalue: numberField,
  min_eigenvalue_cv: optionalField(iv2slsStockYogoCvField),
  min_eigenvalue_cv_note: optionalField(stringField),
});

export const iv2slsFirstStageResultField = objectField<Iv2slsFirstStageResult>({
  endog_name: stringField,
  var_names: arrayField(stringField),
  coefficients: arrayField(coefficientField),
  r_squared: numberField,
  adj_r_squared: numberField,
});
