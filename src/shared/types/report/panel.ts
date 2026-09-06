/**
 * 面板回归报告 DTO
 */

import type { LinearRegressionResultData } from "./regression";

export interface PanelSelectionTest {
  id: string;
  group: "model_choice" | "effect_choice" | string;
  label: string;
  h0: string;
  stat_type: string;
  stat?: number;
  df1?: number;
  df2?: number;
  p_value?: number;
  decision: "significant" | "not_significant" | "unavailable" | string;
  recommendation: string;
  note?: string;
}

export interface PanelSummaryResult {
  title: string;
  endog_name: string;
  mixed_ols?: LinearRegressionResultData;
  fe?: LinearRegressionResultData;
  fe_time?: LinearRegressionResultData;
  fe_twoway?: LinearRegressionResultData;
  lsdv?: LinearRegressionResultData;
  lsdv_time?: LinearRegressionResultData;
  lsdv_twoway?: LinearRegressionResultData;
  fd?: LinearRegressionResultData;
  re_fgls?: LinearRegressionResultData;
  re_mle?: LinearRegressionResultData;
  re_be?: LinearRegressionResultData;
  re_fgls_time?: LinearRegressionResultData;
  re_mle_time?: LinearRegressionResultData;
  re_be_time?: LinearRegressionResultData;
  re_fgls_twoway?: LinearRegressionResultData;
  re_mle_twoway?: LinearRegressionResultData;
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
