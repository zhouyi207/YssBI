/**
 * VEC / vecrank 报告 DTO
 */

import type { VARLmarDisplay, VARStableRow } from './var';

export interface VECSummaryResultData {
  title: string;
  var_names: string[];
  num_observation: number;
  log_likelihood: number;
  aic: number;
  hqic: number;
  sbic: number;
  det_sigma_ml: number;
  rank: number;
  lags: number;
  trend_spec: string;
  equations: VECEquationDisplay[];
  coefficients: VECCoefDisplay[];
  beta: number[][];
  beta_var_names?: string[];
  cointegrating_equations: VECCointegratingEquationDisplay[];
  beta_std_err?: (number | null)[][];
  beta_z_value?: (number | null)[][];
  beta_p_value?: (number | null)[][];
  beta_ci_lower?: (number | null)[][];
  beta_ci_upper?: (number | null)[][];
  veclmar?: VARLmarDisplay[];
  vecstable?: VARStableRow[];
}

export interface VECCointegratingEquationDisplay {
  eq_name: string;
  parms: number;
  chi2: number;
  p_chi2: number;
}

export interface VECEquationDisplay {
  eq_name: string;
  parms: number;
  rmse: number;
  r_sq: number;
  chi2: number;
  p_chi2: number;
}

export interface VECCoefDisplay {
  eq_name: string;
  variable: string;
  coef: number;
  std_err: number;
  z_value: number;
  p_value: number;
  ci_lower: number;
  ci_upper: number;
}

export interface VecRankRowData {
  rank: number;
  log_likelihood: number;
  eigenvalue: number | null;
  trace_statistic: number | null;
  trace_crit_10pct: number | null;
  trace_crit_5pct: number | null;
  trace_crit_1pct: number | null;
  max_eigenvalue_statistic: number | null;
  max_eigen_crit_10pct: number | null;
  max_eigen_crit_5pct: number | null;
  max_eigen_crit_1pct: number | null;
}

export interface VecRankResultData {
  kind: string;
  title: string;
  var_names: string[];
  num_observation: number;
  n_lags: number;
  trend_spec: string;
  show_max_eigen: boolean;
  selected_rank_trace_95: number;
  selected_rank_trace_99: number;
  selected_rank_max_95: number;
  selected_rank_max_99: number;
  rows: VecRankRowData[];
  note: string;
}
