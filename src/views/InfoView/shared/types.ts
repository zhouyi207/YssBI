/** 共享的回归结果类型，供 OLS / Prais / WLS / GLS 等使用 */

export interface ModelBasicInfo {
  model_type: string;
  method: string;
  num_observation: number;
  r_squared: number;
  adj_r_squared: number;
  f_statistic: number;
  prob_f_statistic: number;
  /** IV:2SLS uses Wald chi2 (asymptotic). When present, show Wald chi2 instead of F. */
  wald_chi2?: number;
  prob_wald_chi2?: number;
  /** MLE: log likelihood, LR chi2, chibar2 for sigma_u=0 */
  log_likelihood?: number;
  lr_chi2?: number;
  prob_lr_chi2?: number;
  chibar2?: number;
  prob_chibar2?: number;
  /** MLE: constant-only model iterations (Stata "Fitting constant-only model") */
  mle_iter_log_lik_const?: number[];
  /** MLE: full model iterations (Stata "Fitting full model") */
  mle_iter_log_lik?: number[];
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
  std_err?: number;
  t_value?: number;
  p_value?: number;
  'confidence_interval_0.025'?: number;
  'confidence_interval_0.975'?: number;
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

/** Panel Summary 结果：Mixed(OLS)、FE(Within)、FE(Time)、FE(Two-Way)、LSDV、FD、RE(FGLS/MLE/BE) */
export interface PanelSummaryResult {
  title: string;
  endog_name: string;
  mixed_ols?: OLSResultData;
  fe?: OLSResultData;
  fe_time?: OLSResultData;
  fe_twoway?: OLSResultData;
  lsdv?: OLSResultData;
  lsdv_time?: OLSResultData;
  lsdv_twoway?: OLSResultData;
  fd?: OLSResultData;
  re_fgls?: OLSResultData;
  re_mle?: OLSResultData;
  re_be?: OLSResultData;
  re_fgls_time?: OLSResultData;
  re_mle_time?: OLSResultData;
  re_be_time?: OLSResultData;
  re_fgls_twoway?: OLSResultData;
  re_mle_twoway?: OLSResultData;
  selection_tests?: PanelSelectionTest[];
  errors?: {
    mixed_ols?: string;
    fe?: string;
    fe_time?: string;
    fe_twoway?: string;
    lsdv?: string;
    lsdv_time?: string;
    lsdv_twoway?: string;
    fd?: string;
    re_fgls?: string;
    re_mle?: string;
    re_be?: string;
    re_fgls_time?: string;
    re_mle_time?: string;
    re_be_time?: string;
    re_fgls_twoway?: string;
    re_mle_twoway?: string;
  };
}

export interface PanelSelectionTest {
  id: string;
  group: 'model_choice' | 'effect_choice' | string;
  label: string;
  h0: string;
  stat_type: string;
  stat?: number;
  df1?: number;
  df2?: number;
  p_value?: number;
  decision: 'significant' | 'not_significant' | 'unavailable' | string;
  recommendation: string;
  note?: string;
}

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

export interface OvTest {
  f_stat: number;
  df1: number;
  df2: number;
  p_value: number;
}

export interface OvTests {
  default?: OvTest;
  rhs?: OvTest;
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
  ov_tests_ms?: number;
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

export interface VifEntry {
  variable: string;
  vif: number;
  tolerance: number;
}

export interface DiagnosticInfo {
  cond_no: number;
  vif?: VifEntry[];
  bp_tests?: BreuschPaganTests;
  ov_tests?: OvTests;
  im_test?: ImTest;
  normality_tests?: NormalityTests;
  fitted_values?: number[];
  residuals?: number[];
  /** Leverage（帽子矩阵对角元，Stata predict lev, leverage） */
  leverage?: number[];
  residual_scatter?: ResidualScatterData;
  exog?: number[][];
  timing?: DiagnosticTiming;
  prais_info?: PraisInfo;
  /** IV:2SLS 第一阶段回归结果 */
  iv2sls_first_stage?: Iv2slsFirstStageResult[];
  /** IV:2SLS estat firststage 汇总 */
  iv2sls_first_stage_summary?: Iv2slsFirstStageSummary;
  /** IV:2SLS 过度识别检验（estat overid） */
  iv2sls_overid?: Iv2slsOveridTest;
  /** IV:2SLS 过度识别维度（k_iv=排除的工具变量数, k_endog=内生数） */
  iv2sls_overid_dims?: { k_iv: number; k_endog: number };
  /** IV:2SLS 传统豪斯曼检验（hausman iv ols, constant sigmamore） */
  iv2sls_hausman?: Iv2slsHausmanTest;
  /** IV:2SLS Durbin-Wu-Hausman 内生性检验（estat endogenous） */
  iv2sls_endogenous?: Iv2slsEndogenousTest;
  /** IV:LIML κ (minimum eigenvalue used in κ-class estimator) */
  ivliml_kappa?: number;
  /** IV:LIML 过度识别检验（estat overid）Anderson-Rubin, Basmann F */
  ivliml_overid?: IvLimlOveridTest;
  /** Binary choice (Logit/Probit): classification table (Stata estat classification) */
  classification_table?: ClassificationTable;
  /** Binary choice: mean of each exog column (for margins at means) */
  exog_means?: number[];
  /** Panel FE: Stata xtreg, fe style (R2 Within/Between/Overall, sigma_u, sigma_e, rho, corr, obs per group) */
  panel_fe_info?: PanelFEInfo;
  /** Variables omitted due to strict multicollinearity */
  omit_info?: OmitInfo;
}

export interface OmitInfo {
  omitted: OmittedVariable[];
}

export interface OmittedVariable {
  variable: string;
  category?: string;
  reason: string;
}

/** Panel FE-specific stats (Stata xtreg, fe) */
export interface PanelFEInfo {
  /** R² Within/Between/Overall. undefined for MLE. */
  r2_within?: number;
  r2_between?: number;
  r2_overall?: number;
  num_groups: number;
  obs_per_group: { min: number; avg: number; max: number };
  sigma: { sigma_u: number; sigma_e: number; rho: number };
  corr_u_i_Xb: number;
  /** RE quasi-demeaning parameter θ (min/avg/max across groups) */
  theta?: { min: number; avg: number; max: number };
  /** MLE: chibar2(01) for sigma_u=0 test */
  chibar2?: number;
  prob_chibar2?: number;
}

/** Classification table for binary choice models (Stata estat classification) */
export interface ClassificationTable {
  tp: number;
  fp: number;
  fn_: number;
  tn: number;
  cutoff: number;
  sensitivity: number;
  specificity: number;
  ppv: number;
  npv: number;
  false_pos_rate: number;
  false_neg_rate: number;
  pct_correct: number;
}

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
  /** "robust" | "k_endog_gt_2" when cv is not shown */
  min_eigenvalue_cv_note?: string;
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

export interface Iv2slsFirstStageResult {
  endog_name: string;
  var_names: string[];
  coefficients: Coefficient[];
  r_squared: number;
  adj_r_squared: number;
}

/** varlmar 单行（LM 残差自相关检验，Stata varlmar 命令） */
export interface VARLmarDisplay {
  lag: number;
  chi2: number;
  df: number;
  p_value: number;
}

/** varwle 单行（Wald lag-exclusion，Stata varwle 命令） */
export interface VARWleDisplay {
  eq_name: string;
  lag: number;
  chi2: number;
  df: number;
  p_value: number;
}

/** varstable 单行（特征值平稳性检验，Stata varstable 命令） */
export interface VARStableRow {
  re: number;
  im: number;
  modulus: number;
}

/** vargranger 单行（格兰杰因果 Wald 检验，Stata vargranger 命令） */
export interface VARGrangerDisplay {
  eq_name: string;
  excluded: string;
  chi2: number;
  df: number;
  p_value: number;
}

/** Stata varsoc 表行（Lag 从 0 起；0 无 LR） */
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

/** VAR varsoc 结果（Stata varsoc varlist, maxlag(#)） */
export interface VARSocResultData {
  title: string;
  var_names: string[];
  maxlag: number;
  num_observation: number;
  rows: VARSocRowData[];
}

/** VAR Summary 结果（Stata varbasic 风格） */
export interface VARSummaryResultData {
  title: string;
  var_names: string[];
  /** listwise 后的对齐行数 T；估计用 n = T − var_max_lag */
  complete_sample_rows?: number;
  /** 最大滞后阶 p（Lags 引脚） */
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

/** VEC 协整分析结果（Stata vec 风格） */
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
  /** beta 表变量名，与 beta 列对应（含 const） */
  beta_var_names?: string[];
  cointegrating_equations: VECCointegratingEquationDisplay[];
  /** beta 表 Stata 风格：Std. err., z, P>|z|, [95% conf. interval]，归一化/常数用 null */
  beta_std_err?: (number | null)[][];
  beta_z_value?: (number | null)[][];
  beta_p_value?: (number | null)[][];
  beta_ci_lower?: (number | null)[][];
  beta_ci_upper?: (number | null)[][];
  /** veclmar: LM 残差自相关检验 */
  veclmar?: VARLmarDisplay[];
  /** vecstable: 特征值平稳性检验 */
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

/** Stata vecrank 风格 — Johansen trace / max eigenvalue */
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

/** DF & ADF 回归表行 */
export interface DFADFRegRowData {
  variable: string;
  coef: number;
  std_err: number;
  t: number;
  p_value: number;
  ci_lower: number;
  ci_upper: number;
}

/** DF & ADF 单位根检验结果（Stata dfuller） */
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

/** DF & ADF Summary 列表结果：遍历 constant/trend/lags 所有组合 */
export interface DFADFSummaryListResultData {
  title: string;
  var_name: string;
  items: DFADFSummaryResultData[];
}
