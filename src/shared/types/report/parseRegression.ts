import type { Coefficient, RegressionResultData } from "./regression";
import { coefficientField, linearModelInfoField, binaryModelInfoField } from "./parseCommon";
import {
  numberField,
  stringField,
  booleanField,
  literalField,
  optionalField,
  nullableField,
  arrayField,
  objectField,
  refineField,
  type ReportField,
  type ReportFieldIssue,
} from "./fields";
import type {
  DiagnosticInfo,
  VifEntry,
  BreuschPaganTests,
  BreuschPaganTest,
  OvTests,
  OvTest,
  ImTest,
  ImTestComponent,
  NormalityTests,
  ResidualScatterData,
  DiagnosticTiming,
  PraisInfo,
  ClassificationTable,
  PanelFEInfo,
  OmitInfo,
  OmittedVariable,
  BinaryModelStatistics,
} from "./regression";
import type { PlotPointDTO } from "@/shared/types/domain/plotPayload";
import {
  iv2slsFirstStageResultField,
  iv2slsFirstStageSummaryField,
  iv2slsOveridTestField,
  iv2slsHausmanTestField,
  iv2slsEndogenousTestField,
  ivLimlOveridTestField,
} from "./iv";

const vifEntryField = objectField<VifEntry>({
  variable: stringField,
  category: optionalField(nullableField(stringField)),
  vif: numberField,
  tolerance: numberField,
});

const breuschPaganTestField = objectField<BreuschPaganTest>({
  lm_stat: numberField,
  df: numberField,
  p_value: numberField,
});

const breuschPaganTestsField = objectField<BreuschPaganTests>({
  stata: optionalField(breuschPaganTestField),
  koenker: optionalField(breuschPaganTestField),
  stata_rhs: optionalField(breuschPaganTestField),
  koenker_rhs: optionalField(breuschPaganTestField),
});

const ovTestField = objectField<OvTest>({
  f_stat: numberField,
  df1: numberField,
  df2: numberField,
  p_value: numberField,
});

const ovTestsField = objectField<OvTests>({
  default: optionalField(ovTestField),
  rhs: optionalField(ovTestField),
});

const imTestComponentField = objectField<ImTestComponent>({
  chi2: numberField,
  df: numberField,
  p_value: numberField,
});

const imTestField = objectField<ImTest>({
  heteroskedasticity: imTestComponentField,
  skewness: imTestComponentField,
  kurtosis: imTestComponentField,
  total: imTestComponentField,
});

const normalityTestsField = objectField<NormalityTests>({
  skewness: numberField,
  kurtosis: numberField,
  omnibus_stat: numberField,
  omnibus_p_value: numberField,
  jarque_bera_stat: numberField,
  jarque_bera_p_value: numberField,
});

const plotPointDTOField = objectField<PlotPointDTO>({
  x: numberField,
  y: numberField,
});

const residualScatterDataField = objectField<ResidualScatterData>({
  e: arrayField(numberField),
  e_lag1: arrayField(numberField),
  time: optionalField(arrayField(stringField)),
});

const diagnosticTimingField = objectField<DiagnosticTiming>({
  fitted_residuals_ms: optionalField(numberField),
  bp_tests_ms: optionalField(numberField),
  ov_tests_ms: optionalField(numberField),
  im_test_ms: optionalField(numberField),
});

const praisInfoField = objectField<PraisInfo>({
  rho: numberField,
  dw_original: numberField,
  dw_transformed: numberField,
  iterations: numberField,
  iteration_log: optionalField(arrayField(stringField)),
});

const classificationTableField = objectField<ClassificationTable>({
  tp: numberField,
  fp: numberField,
  fn_: numberField,
  tn: numberField,
  cutoff: numberField,
  sensitivity: numberField,
  specificity: numberField,
  ppv: numberField,
  npv: numberField,
  false_pos_rate: numberField,
  false_neg_rate: numberField,
  pct_correct: numberField,
});

const panelFEInfoField = objectField<PanelFEInfo>({
  r2_within: optionalField(numberField),
  r2_between: optionalField(numberField),
  r2_overall: optionalField(numberField),
  num_groups: numberField,
  obs_per_group: objectField({
    min: numberField,
    avg: numberField,
    max: numberField,
  }),
  sigma: objectField({
    sigma_u: numberField,
    sigma_e: numberField,
    rho: numberField,
  }),
  corr_u_i_Xb: numberField,
  theta: optionalField(
    objectField({
      min: numberField,
      avg: numberField,
      max: numberField,
    }),
  ),
  chibar2: optionalField(numberField),
  prob_chibar2: optionalField(numberField),
});

const omittedVariableField = objectField<OmittedVariable>({
  variable: stringField,
  category: optionalField(stringField),
  reason: stringField,
});

const omitInfoField = objectField<OmitInfo>({
  omitted: arrayField(omittedVariableField),
});

const diagnosticInfoField = objectField<DiagnosticInfo>({
  cond_no: numberField,
  vif: optionalField(arrayField(vifEntryField)),
  bp_tests: optionalField(breuschPaganTestsField),
  ov_tests: optionalField(ovTestsField),
  im_test: optionalField(imTestField),
  normality_tests: optionalField(normalityTestsField),
  fitted_values: optionalField(arrayField(numberField)),
  residuals: optionalField(arrayField(numberField)),
  leverage: optionalField(arrayField(numberField)),
  leverage_kde: optionalField(arrayField(plotPointDTOField)),
  residual_scatter: optionalField(residualScatterDataField),
  exog: optionalField(arrayField(arrayField(numberField))),
  timing: optionalField(diagnosticTimingField),
  prais_info: optionalField(praisInfoField),
  iv2sls_first_stage: optionalField(arrayField(iv2slsFirstStageResultField)),
  iv2sls_first_stage_summary: optionalField(iv2slsFirstStageSummaryField),
  iv2sls_overid: optionalField(iv2slsOveridTestField),
  iv2sls_overid_dims: optionalField(
    objectField({
      k_iv: numberField,
      k_endog: numberField,
    }),
  ),
  iv2sls_hausman: optionalField(iv2slsHausmanTestField),
  iv2sls_endogenous: optionalField(iv2slsEndogenousTestField),
  ivliml_kappa: optionalField(numberField),
  ivliml_overid: optionalField(ivLimlOveridTestField),
  classification_table: optionalField(classificationTableField),
  exog_means: optionalField(arrayField(numberField)),
  panel_fe_info: optionalField(panelFEInfoField),
  omit_info: optionalField(omitInfoField),
});

const binaryModelStatisticsField = objectField<BinaryModelStatistics>({
  kind: literalField("binary"),
  link: literalField("logit", "probit"),
  covariance: arrayField(arrayField(numberField)),
  standardErrors: arrayField(numberField),
  statisticValues: arrayField(numberField),
  pValues: arrayField(numberField),
  confidenceIntervalLower: arrayField(numberField),
  confidenceIntervalUpper: arrayField(numberField),
  logLikelihood: numberField,
  nullLogLikelihood: numberField,
  pseudoR2: numberField,
  adjustedPseudoR2: numberField,
  lrChi2: numberField,
  lrPValue: numberField,
  aic: numberField,
  bic: numberField,
  iterations: numberField,
  converged: booleanField,
  conditionNumber: numberField,
});

function matrixSize(matrix: number[][], size: number): boolean {
  return matrix.length === size && matrix.every((row) => row.length === size);
}
function validateRegression<ModelInfo>(
  value: RegressionResultData<ModelInfo>,
): ReportFieldIssue | null {
  const { betas, cov_beta: covariance, model_statistics: statistics, coefficients } = value;
  const issue = (fieldPath: string): ReportFieldIssue => ({
    fieldPath,
    reason: "inconsistent regression dimensions or covariance",
  });
  if (betas && betas.length !== coefficients.length) return issue("betas");
  if (covariance && !matrixSize(covariance, betas?.length ?? covariance.length))
    return issue("cov_beta");
  if (statistics) {
    const size = coefficients.length;
    if (
      !matrixSize(statistics.covariance, size) ||
      [
        statistics.standardErrors,
        statistics.statisticValues,
        statistics.pValues,
        statistics.confidenceIntervalLower,
        statistics.confidenceIntervalUpper,
      ].some((values) => values.length !== size)
    )
      return issue("model_statistics");
    if (
      covariance &&
      covariance.some((row, i) => row.some((value, j) => value !== statistics.covariance[i][j]))
    )
      return issue("cov_beta");
  }
  return null;
}
function regressionReportField<ModelInfo>(
  model: ReportField<ModelInfo>,
  coefficient: ReportField<Coefficient> = coefficientField,
  title: ReportField<string> = stringField,
): ReportField<RegressionResultData<ModelInfo>> {
  return refineField(
    objectField<RegressionResultData<ModelInfo>>({
      title,
      endog_name: optionalField(stringField),
      model_basic_info: model,
      coefficients: arrayField(coefficient),
      diagnostic_info: diagnosticInfoField,
      betas: optionalField(arrayField(numberField)),
      cov_beta: optionalField(arrayField(arrayField(numberField))),
      model_statistics: optionalField(binaryModelStatisticsField),
      executionTimeMs: optionalField(numberField),
    }),
    validateRegression,
  );
}
export const linearRegressionReportField = regressionReportField(linearModelInfoField);
export const binaryRegressionReportField = regressionReportField(binaryModelInfoField);
const olsCoefficientField = objectField<Coefficient>({
  variable: stringField,
  category: optionalField(stringField),
  coef: numberField,
  std_err: numberField,
  t_value: numberField,
  p_value: numberField,
  "confidence_interval_0.025": numberField,
  "confidence_interval_0.975": numberField,
  is_significant: booleanField,
});
export const canonicalOlsReportField = regressionReportField(
  linearModelInfoField,
  olsCoefficientField,
  literalField("OLS Summary"),
);
