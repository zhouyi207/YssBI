import {
  numberField,
  integerField,
  stringField,
  booleanField,
  optionalField,
  nullableField,
  arrayField,
  objectField,
} from "./fields";
import type {
  VECSummaryResultData,
  VECCointegratingEquationDisplay,
  VecRankResultData,
  VecRankRowData,
} from "./vec";
import {
  varEquationDisplayField,
  varCoefDisplayField,
  varLmarDisplayField,
  varStableRowField,
} from "./parseVar";

const vecCointegratingEquationDisplayField = objectField<VECCointegratingEquationDisplay>({
  eq_name: stringField,
  parms: numberField,
  chi2: numberField,
  p_chi2: numberField,
});

export const vecSummaryResultDataField = objectField<VECSummaryResultData>({
  title: stringField,
  var_names: arrayField(stringField),
  num_observation: numberField,
  log_likelihood: numberField,
  aic: numberField,
  hqic: numberField,
  sbic: numberField,
  det_sigma_ml: numberField,
  rank: integerField,
  lags: integerField,
  trend_spec: stringField,
  equations: arrayField(varEquationDisplayField),
  coefficients: arrayField(varCoefDisplayField),
  beta: arrayField(arrayField(numberField)),
  beta_var_names: optionalField(arrayField(stringField)),
  cointegrating_equations: arrayField(vecCointegratingEquationDisplayField),
  beta_std_err: optionalField(arrayField(arrayField(nullableField(numberField)))),
  beta_z_value: optionalField(arrayField(arrayField(nullableField(numberField)))),
  beta_p_value: optionalField(arrayField(arrayField(nullableField(numberField)))),
  beta_ci_lower: optionalField(arrayField(arrayField(nullableField(numberField)))),
  beta_ci_upper: optionalField(arrayField(arrayField(nullableField(numberField)))),
  veclmar: optionalField(arrayField(varLmarDisplayField)),
  vecstable: optionalField(arrayField(varStableRowField)),
});

const vecRankRowDataField = objectField<VecRankRowData>({
  rank: numberField,
  log_likelihood: numberField,
  eigenvalue: nullableField(numberField),
  trace_statistic: nullableField(numberField),
  trace_crit_10pct: nullableField(numberField),
  trace_crit_5pct: nullableField(numberField),
  trace_crit_1pct: nullableField(numberField),
  max_eigenvalue_statistic: nullableField(numberField),
  max_eigen_crit_10pct: nullableField(numberField),
  max_eigen_crit_5pct: nullableField(numberField),
  max_eigen_crit_1pct: nullableField(numberField),
});

export const vecRankResultDataField = objectField<VecRankResultData>({
  kind: stringField,
  title: stringField,
  var_names: arrayField(stringField),
  num_observation: numberField,
  n_lags: integerField,
  trend_spec: stringField,
  show_max_eigen: booleanField,
  selected_rank_trace_95: integerField,
  selected_rank_trace_99: integerField,
  selected_rank_max_95: integerField,
  selected_rank_max_99: integerField,
  rows: arrayField(vecRankRowDataField),
  note: stringField,
});
