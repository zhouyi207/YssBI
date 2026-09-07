import {
  numberField,
  stringField,
  booleanField,
  optionalField,
  arrayField,
  objectField,
  readReportField,
  parsedField,
} from "./fields";
import type { Coefficient, LinearModelInfo, BinaryModelInfo } from "./regression";

export const coefficientField = objectField<Coefficient>({
  variable: stringField,
  category: optionalField(stringField),
  coef: numberField,
  std_err: optionalField(numberField),
  t_value: optionalField(numberField),
  p_value: optionalField(numberField),
  "confidence_interval_0.025": optionalField(numberField),
  "confidence_interval_0.975": optionalField(numberField),
  is_significant: booleanField,
});

export const linearModelInfoField = objectField<LinearModelInfo>({
  model_type: stringField,
  method: stringField,
  num_observation: numberField,
  r_squared: numberField,
  adj_r_squared: numberField,
  f_statistic: numberField,
  prob_f_statistic: numberField,
  wald_chi2: optionalField(numberField),
  prob_wald_chi2: optionalField(numberField),
  log_likelihood: optionalField(numberField),
  lr_chi2: optionalField(numberField),
  prob_lr_chi2: optionalField(numberField),
  chibar2: optionalField(numberField),
  prob_chibar2: optionalField(numberField),
  mle_iter_log_lik_const: optionalField(arrayField(numberField)),
  mle_iter_log_lik: optionalField(arrayField(numberField)),
  df_model: numberField,
  df_residual: numberField,
  df_total: numberField,
  ss_model: numberField,
  ss_residual: numberField,
  ss_total: numberField,
  ms_model: numberField,
  ms_residual: numberField,
  ms_total: numberField,
  covariance_type: stringField,
  aic: optionalField(numberField),
  bic: optionalField(numberField),
});

export const binaryModelInfoField = objectField<BinaryModelInfo>({
  model_type: stringField,
  method: stringField,
  num_observation: numberField,
  pseudo_r2: numberField,
  adjusted_pseudo_r2: numberField,
  log_likelihood: numberField,
  lr_chi2: numberField,
  prob_lr_chi2: numberField,
  df_model: numberField,
  df_residual: numberField,
  covariance_type: stringField,
  aic: numberField,
  bic: numberField,
});

export function parseCoefficient(raw: unknown) {
  return readReportField(coefficientField, raw);
}
export function parseCoefficientList(raw: unknown) {
  return readReportField(arrayField(coefficientField), raw);
}
export function parseLinearModelInfo(raw: unknown) {
  return readReportField(linearModelInfoField, raw);
}
export function parseBinaryModelInfo(raw: unknown) {
  return readReportField(binaryModelInfoField, raw);
}
export function parseFiniteNumberArray(raw: unknown) {
  return readReportField(arrayField(numberField), raw);
}
export function parseStringArray(raw: unknown) {
  return readReportField(arrayField(stringField), raw);
}
export function parseObjectArray<T>(
  raw: unknown,
  parseItem: (raw: unknown) => T | null,
): T[] | null {
  return readReportField(arrayField(parsedField(parseItem, "report object")), raw);
}
