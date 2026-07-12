/**
 * 面板 DID 报告 DTO
 */

import type { OLSResultData } from './regression';

export interface DidEventStudyPoint {
  rel_time: number;
  coef: number;
  std_err: number;
  ci_low: number;
  ci_high: number;
  is_reference?: boolean;
}

export interface DidParallelTrendsBlock {
  available: boolean;
  chi2?: number;
  df?: number;
  p_value?: number;
  reference_rel?: number;
  tested_rel_periods?: number[];
  event_study?: DidEventStudyPoint[];
  method_note: string;
}

export interface DidPlaceboTimingBlock {
  available: boolean;
  coef?: number;
  std_err?: number;
  t_value?: number;
  p_value?: number;
  horizon: number;
  method_note: string;
}

export interface DidPlaceboFakeGroupBlock {
  available: boolean;
  observed_coef?: number;
  n_perm: number;
  n_perm_valid: number;
  p_value_ri?: number;
  perm_coef_mean?: number;
  perm_coef_std?: number;
  method_note: string;
}

export interface ExogLabelEntry {
  variable: string;
  category?: string | null;
}

export interface DidFakeGroupEnginePayload {
  endog: number[];
  exog_row_major: number[];
  ncols: number;
  all_labels: ExogLabelEntry[];
  entity_id: number[];
  time_id: number[];
  post: number[];
  treat: number[];
  did_label: string;
  observed_coef: number;
  constant: boolean;
  cov_type: string;
}

export interface PanelDidResultData {
  kind: 'panel_did';
  title: string;
  endog_name: string;
  treat_name: string;
  post_name: string;
  fe_twoway?: OLSResultData;
  error?: string;
  parallel_trends?: DidParallelTrendsBlock;
  placebo?: DidPlaceboTimingBlock;
  fake_group_engine?: DidFakeGroupEnginePayload | null;
  placebo_fake_group?: DidPlaceboFakeGroupBlock;
}
