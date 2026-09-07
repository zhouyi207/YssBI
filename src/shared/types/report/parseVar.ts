import {
  numberField,
  integerField,
  stringField,
  optionalField,
  nullableField,
  arrayField,
  objectField,
  readReportField,
} from "./fields";
import type {
  VARSocResultData,
  VARSocRowData,
  VARSummaryResultData,
  VAREquationDisplay,
  VARCoefDisplay,
  VARWleDisplay,
  VARLmarDisplay,
  VARStableRow,
  VARGrangerDisplay,
} from "./var";

const varSocRowDataField = objectField<VARSocRowData>({
  lag: numberField,
  log_likelihood: numberField,
  lr: optionalField(nullableField(numberField)),
  lr_df: optionalField(nullableField(numberField)),
  lr_p: optionalField(nullableField(numberField)),
  fpe: numberField,
  aic: numberField,
  hqic: numberField,
  sbic: numberField,
});

export const varSocResultDataField = objectField<VARSocResultData>({
  title: stringField,
  var_names: arrayField(stringField),
  maxlag: integerField,
  num_observation: numberField,
  rows: arrayField(varSocRowDataField),
});

export const varEquationDisplayField = objectField<VAREquationDisplay>({
  eq_name: stringField,
  parms: numberField,
  rmse: numberField,
  r_sq: numberField,
  chi2: numberField,
  p_chi2: numberField,
});

export const varCoefDisplayField = objectField<VARCoefDisplay>({
  eq_name: stringField,
  variable: stringField,
  coef: numberField,
  std_err: numberField,
  z_value: numberField,
  p_value: numberField,
  ci_lower: numberField,
  ci_upper: numberField,
});

const varWleDisplayField = objectField<VARWleDisplay>({
  eq_name: stringField,
  lag: integerField,
  chi2: numberField,
  df: numberField,
  p_value: numberField,
});

export const varLmarDisplayField = objectField<VARLmarDisplay>({
  lag: integerField,
  chi2: numberField,
  df: numberField,
  p_value: numberField,
});

export const varStableRowField = objectField<VARStableRow>({
  re: numberField,
  im: numberField,
  modulus: numberField,
});

const varGrangerDisplayField = objectField<VARGrangerDisplay>({
  eq_name: stringField,
  excluded: stringField,
  chi2: numberField,
  df: numberField,
  p_value: numberField,
});

export const varSummaryResultDataField = objectField<VARSummaryResultData>({
  title: stringField,
  var_names: arrayField(stringField),
  complete_sample_rows: optionalField(integerField),
  var_max_lag: optionalField(integerField),
  num_observation: numberField,
  log_likelihood: numberField,
  aic: numberField,
  fpe: numberField,
  hqic: numberField,
  sbic: numberField,
  det_sigma_ml: numberField,
  equations: arrayField(varEquationDisplayField),
  coefficients: arrayField(varCoefDisplayField),
  sigma: arrayField(arrayField(numberField)),
  oirf: arrayField(arrayField(arrayField(numberField))),
  fevd: arrayField(arrayField(arrayField(numberField))),
  varwle: optionalField(arrayField(varWleDisplayField)),
  varlmar: optionalField(arrayField(varLmarDisplayField)),
  varstable: optionalField(arrayField(varStableRowField)),
  vargranger: optionalField(arrayField(varGrangerDisplayField)),
});

export function parseVarSocResultData(raw: unknown) {
  return readReportField(varSocResultDataField, raw);
}
export function parseVarSummaryResultData(raw: unknown) {
  return readReportField(varSummaryResultDataField, raw);
}
