/** 共享的回归结果类型，供 OLS / Prais / WLS / GLS 等使用 */

export interface ModelBasicInfo {
  model_type: string;
  method: string;
  num_observation: number;
  r_squared: number;
  adj_r_squared: number;
  f_statistic: number;
  prob_f_statistic: number;
  df_model: number;
  df_residual: number;
  df_total: number;
  ss_model: number;
  ss_residual: number;
  ss_total: number;
  ms_model: number;
  ms_residual: number;
  ms_total: number;
  covariance_type: string;
  aic?: number;
  bic?: number;
}

export interface Coefficient {
  variable: string;
  category?: string;
  coef: number;
  std_err: number;
  t_value: number;
  p_value: number;
  'confidence_interval_0.025': number;
  'confidence_interval_0.975': number;
  is_significant: boolean;
}

/** 回归结果通用结构（OLS / Prais / WLS / GLS 共用） */
export interface RegressionResultData {
  title: string;
  endog_name?: string;
  model_basic_info: ModelBasicInfo;
  coefficients: Coefficient[];
  diagnostic_info: DiagnosticInfo;
  betas?: number[];
  cov_beta?: number[][];
  executionTimeMs?: number;
}

/** OLS 专用别名，与后端 OLSResult 一致 */
export type OLSResultData = RegressionResultData;

export interface BreuschPaganTest {
  lm_stat: number;
  df: number;
  p_value: number;
}

export interface BreuschPaganTests {
  stata?: BreuschPaganTest;
  koenker?: BreuschPaganTest;
  stata_rhs?: BreuschPaganTest;
  koenker_rhs?: BreuschPaganTest;
}

export interface ImTestComponent {
  chi2: number;
  df: number;
  p_value: number;
}

export interface ImTest {
  heteroskedasticity: ImTestComponent;
  skewness: ImTestComponent;
  kurtosis: ImTestComponent;
  total: ImTestComponent;
}

export interface NormalityTests {
  skewness: number;
  kurtosis: number;
  omnibus_stat: number;
  omnibus_p_value: number;
  jarque_bera_stat: number;
  jarque_bera_p_value: number;
}

export interface DiagnosticTiming {
  fitted_residuals_ms?: number;
  bp_tests_ms?: number;
  im_test_ms?: number;
}

export interface ResidualScatterData {
  e: number[];
  e_lag1: number[];
  time?: string[];
}

export interface PraisInfo {
  rho: number;
  dw_original: number;
  dw_transformed: number;
  iterations: number;
  /** Iteration log: "Prais iteration N: rho = X.XXXX" for each step */
  iteration_log?: string[];
}

export interface DiagnosticInfo {
  cond_no: number;
  bp_tests?: BreuschPaganTests;
  im_test?: ImTest;
  normality_tests?: NormalityTests;
  fitted_values?: number[];
  residuals?: number[];
  residual_scatter?: ResidualScatterData;
  exog?: number[][];
  timing?: DiagnosticTiming;
  prais_info?: PraisInfo;
}
