/**
 * DF / ADF 单位根检验报告 DTO
 */

export interface DFADFRegRowData {
  variable: string;
  coef: number;
  std_err: number;
  t: number;
  p_value: number;
  ci_lower: number;
  ci_upper: number;
}

export interface DFADFSummaryResultData {
  title: string;
  var_name: string;
  h0: string;
  test_statistic: number;
  critical_value_1pct: number;
  critical_value_5pct: number;
  critical_value_10pct: number;
  p_value: number;
  use_t_distribution: boolean;
  num_obs: number;
  lags: number;
  regression: string;
  coef_lagged: number;
  std_err_lagged: number;
  regression_table: DFADFRegRowData[];
}

export interface DFADFSummaryListResultData {
  title: string;
  var_name: string;
  items: DFADFSummaryResultData[];
}
