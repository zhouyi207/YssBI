/**
 * 回归报告 DTO（OLS / Prais / WLS / GLS / Logit / Probit 等共用）
 * 对齐 Rust `info_nodes.rs` 中 `RegressionResult` / `DiagnosticInfo` 等结构。
 */

import type { PlotPointDTO } from '@/shared/types/dto/plotPayload';
import type {
  Iv2slsEndogenousTest,
  Iv2slsFirstStageResult,
  Iv2slsFirstStageSummary,
  Iv2slsHausmanTest,
  Iv2slsOveridTest,
  IvLimlOveridTest,
} from './iv';

export interface ModelBasicInfo {
  model_type: string;
  method: string;
  num_observation: number;
  r_squared: number;
  adj_r_squared: number;
  f_statistic: number;
  prob_f_statistic: number;
  wald_chi2?: number;
  prob_wald_chi2?: number;
  log_likelihood?: number;
  lr_chi2?: number;
  prob_lr_chi2?: number;
  chibar2?: number;
  prob_chibar2?: number;
  mle_iter_log_lik_const?: number[];
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
  iteration_log?: string[];
}

export interface VifEntry {
  variable: string;
  category?: string | null;
  vif: number;
  tolerance: number;
}

export interface OmitInfo {
  omitted: OmittedVariable[];
}

export interface OmittedVariable {
  variable: string;
  category?: string;
  reason: string;
}

export interface PanelFEInfo {
  r2_within?: number;
  r2_between?: number;
  r2_overall?: number;
  num_groups: number;
  obs_per_group: { min: number; avg: number; max: number };
  sigma: { sigma_u: number; sigma_e: number; rho: number };
  corr_u_i_Xb: number;
  theta?: { min: number; avg: number; max: number };
  chibar2?: number;
  prob_chibar2?: number;
}

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

export interface DiagnosticInfo {
  cond_no: number;
  vif?: VifEntry[];
  bp_tests?: BreuschPaganTests;
  ov_tests?: OvTests;
  im_test?: ImTest;
  normality_tests?: NormalityTests;
  fitted_values?: number[];
  residuals?: number[];
  leverage?: number[];
  leverage_kde?: PlotPointDTO[];
  residual_scatter?: ResidualScatterData;
  exog?: number[][];
  timing?: DiagnosticTiming;
  prais_info?: PraisInfo;
  iv2sls_first_stage?: Iv2slsFirstStageResult[];
  iv2sls_first_stage_summary?: Iv2slsFirstStageSummary;
  iv2sls_overid?: Iv2slsOveridTest;
  iv2sls_overid_dims?: { k_iv: number; k_endog: number };
  iv2sls_hausman?: Iv2slsHausmanTest;
  iv2sls_endogenous?: Iv2slsEndogenousTest;
  ivliml_kappa?: number;
  ivliml_overid?: IvLimlOveridTest;
  classification_table?: ClassificationTable;
  exog_means?: number[];
  panel_fe_info?: PanelFEInfo;
  omit_info?: OmitInfo;
}

export interface BinaryModelStatistics {
  kind: 'binary';
  link: 'logit' | 'probit';
  covariance: number[][];
  standardErrors: number[];
  statisticValues: number[];
  pValues: number[];
  confidenceIntervalLower: number[];
  confidenceIntervalUpper: number[];
  logLikelihood: number;
  nullLogLikelihood: number;
  pseudoR2: number;
  adjustedPseudoR2: number;
  lrChi2: number;
  lrPValue: number;
  aic: number;
  bic: number;
  iterations: number;
  converged: boolean;
  conditionNumber: number;
}

export interface RegressionResultData {
  title: string;
  endog_name?: string;
  model_basic_info: ModelBasicInfo;
  coefficients: Coefficient[];
  diagnostic_info: DiagnosticInfo;
  betas?: number[];
  cov_beta?: number[][];
  model_statistics?: BinaryModelStatistics;
  executionTimeMs?: number;
}

export type OLSResultData = RegressionResultData;
