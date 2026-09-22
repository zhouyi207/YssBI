import {
  numberField,
  stringField,
  booleanField,
  literalField,
  optionalField,
  nullableField,
  arrayField,
  objectField,
  parsedField,
  refineField,
} from "./fields";
import type { PanelSummaryResult, PanelSelectionTest } from "./panel";
import type {
  PanelDidResultData,
  DidParallelTrendsBlock,
  DidEventStudyPoint,
  DidPlaceboTimingBlock,
  DidFakeGroupEnginePayload,
  ExogLabelEntry,
} from "./did";
import { inlineRegressionReportField } from "./parseRegression";
import { parseDidPlaceboFakeGroupBlock } from "./did";

const panelSelectionTestField = objectField<PanelSelectionTest>({
  id: stringField,
  group: stringField,
  label: stringField,
  h0: stringField,
  stat_type: stringField,
  stat: optionalField(numberField),
  df1: optionalField(numberField),
  df2: optionalField(numberField),
  p_value: optionalField(numberField),
  decision: stringField,
  recommendation: stringField,
  note: optionalField(stringField),
});

const panelSummaryResultField = objectField<PanelSummaryResult>({
  title: stringField,
  endog_name: stringField,
  mixed_ols: optionalField(inlineRegressionReportField),
  fe: optionalField(inlineRegressionReportField),
  fe_time: optionalField(inlineRegressionReportField),
  fe_twoway: optionalField(inlineRegressionReportField),
  lsdv: optionalField(inlineRegressionReportField),
  lsdv_time: optionalField(inlineRegressionReportField),
  lsdv_twoway: optionalField(inlineRegressionReportField),
  fd: optionalField(inlineRegressionReportField),
  re_fgls: optionalField(inlineRegressionReportField),
  re_mle: optionalField(inlineRegressionReportField),
  re_be: optionalField(inlineRegressionReportField),
  re_fgls_time: optionalField(inlineRegressionReportField),
  re_mle_time: optionalField(inlineRegressionReportField),
  re_be_time: optionalField(inlineRegressionReportField),
  re_fgls_twoway: optionalField(inlineRegressionReportField),
  re_mle_twoway: optionalField(inlineRegressionReportField),
  selection_tests: optionalField(arrayField(panelSelectionTestField)),
  errors: optionalField(
    objectField({
      mixed_ols: optionalField(stringField),
      fe: optionalField(stringField),
      fe_time: optionalField(stringField),
      fe_twoway: optionalField(stringField),
      lsdv: optionalField(stringField),
      lsdv_time: optionalField(stringField),
      lsdv_twoway: optionalField(stringField),
      fd: optionalField(stringField),
      re_fgls: optionalField(stringField),
      re_mle: optionalField(stringField),
      re_be: optionalField(stringField),
      re_fgls_time: optionalField(stringField),
      re_mle_time: optionalField(stringField),
      re_be_time: optionalField(stringField),
      re_fgls_twoway: optionalField(stringField),
      re_mle_twoway: optionalField(stringField),
    }),
  ),
});

const didEventStudyPointField = objectField<DidEventStudyPoint>({
  rel_time: numberField,
  coef: numberField,
  std_err: numberField,
  ci_low: numberField,
  ci_high: numberField,
  is_reference: optionalField(booleanField),
});

const didParallelTrendsBlockField = objectField<DidParallelTrendsBlock>({
  available: booleanField,
  chi2: optionalField(numberField),
  df: optionalField(numberField),
  p_value: optionalField(numberField),
  reference_rel: optionalField(numberField),
  tested_rel_periods: optionalField(arrayField(numberField)),
  event_study: optionalField(arrayField(didEventStudyPointField)),
  method_note: stringField,
});

const didPlaceboTimingBlockField = objectField<DidPlaceboTimingBlock>({
  available: booleanField,
  coef: optionalField(numberField),
  std_err: optionalField(numberField),
  t_value: optionalField(numberField),
  p_value: optionalField(numberField),
  horizon: numberField,
  method_note: stringField,
});

const exogLabelEntryField = objectField<ExogLabelEntry>({
  variable: stringField,
  category: optionalField(nullableField(stringField)),
});

const didFakeGroupEnginePayloadField = objectField<DidFakeGroupEnginePayload>({
  endog: arrayField(numberField),
  exog_row_major: arrayField(numberField),
  ncols: numberField,
  all_labels: arrayField(exogLabelEntryField),
  entity_id: arrayField(numberField),
  time_id: arrayField(numberField),
  post: arrayField(numberField),
  treat: arrayField(numberField),
  did_label: stringField,
  observed_coef: numberField,
  constant: booleanField,
  cov_type: stringField,
});

export const panelDidResultDataField = objectField<PanelDidResultData>({
  kind: literalField("panel_did"),
  title: stringField,
  endog_name: stringField,
  treat_name: stringField,
  post_name: stringField,
  fe_twoway: optionalField(inlineRegressionReportField),
  error: optionalField(stringField),
  parallel_trends: optionalField(didParallelTrendsBlockField),
  placebo: optionalField(didPlaceboTimingBlockField),
  fake_group_engine: optionalField(nullableField(didFakeGroupEnginePayloadField)),
  placebo_fake_group: optionalField(
    parsedField(parseDidPlaceboFakeGroupBlock, "DID fake-group result"),
  ),
});

export const panelSummaryField = refineField(panelSummaryResultField, (value) =>
  value.selection_tests === undefined &&
  !Object.entries(value).some(
    ([key, model]) =>
      key !== "title" && key !== "endog_name" && key !== "errors" && model !== undefined,
  )
    ? { fieldPath: "$", reason: "expected a panel model or selection tests" }
    : null,
);
