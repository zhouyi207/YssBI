/**
 * 面板回归报告 DTO
 */

import type { OLSResultData } from './regression';

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
