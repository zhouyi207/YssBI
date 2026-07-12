/**
 * VAR / varsoc 报告 DTO
 */

export interface VARLmarDisplay {
  lag: number;
  chi2: number;
  df: number;
  p_value: number;
}

export interface VARWleDisplay {
  eq_name: string;
  lag: number;
  chi2: number;
  df: number;
  p_value: number;
}

export interface VARStableRow {
  re: number;
  im: number;
  modulus: number;
}

export interface VARGrangerDisplay {
  eq_name: string;
  excluded: string;
  chi2: number;
  df: number;
  p_value: number;
}

export interface VARSocRowData {
  lag: number;
  log_likelihood: number;
  lr?: number | null;
  lr_df?: number | null;
  lr_p?: number | null;
  fpe: number;
  aic: number;
  hqic: number;
  sbic: number;
}

export interface VARSocResultData {
  title: string;
  var_names: string[];
  maxlag: number;
  num_observation: number;
  rows: VARSocRowData[];
}

export interface VARSummaryResultData {
  title: string;
  var_names: string[];
  complete_sample_rows?: number;
  var_max_lag?: number;
  num_observation: number;
  log_likelihood: number;
  aic: number;
  fpe: number;
  hqic: number;
  sbic: number;
  det_sigma_ml: number;
  equations: VAREquationDisplay[];
  coefficients: VARCoefDisplay[];
  sigma: number[][];
  oirf: number[][][];
  fevd: number[][][];
  varwle?: VARWleDisplay[];
  varlmar?: VARLmarDisplay[];
  varstable?: VARStableRow[];
  vargranger?: VARGrangerDisplay[];
}

export interface VAREquationDisplay {
  eq_name: string;
  parms: number;
  rmse: number;
  r_sq: number;
  chi2: number;
  p_chi2: number;
}

export interface VARCoefDisplay {
  eq_name: string;
  variable: string;
  coef: number;
  std_err: number;
  z_value: number;
  p_value: number;
  ci_lower: number;
  ci_upper: number;
}
