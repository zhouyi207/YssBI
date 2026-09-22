import type {
  LinearRegressionReportData,
  LinearSummaryOptions,
  ResultAnalysis,
} from "@/shared/types/domain/resultReport";
import type { ResultReference } from "@/shared/types/domain/result";
import type { Coefficient } from "@/shared/types/report/regression";
import { keyValueDataField, tableDataField, statCardDataField } from "./parseUiData";
import {
  arrayField,
  booleanField,
  integerField,
  literalField,
  numberField,
  objectField,
  optionalField,
  refineField,
  stringField,
  type ReportField,
} from "@/shared/types/report/fields";
import { isUuid } from "@/shared/types/domain/editorProjectionGuards";

export const resultReferenceField = refineField(
  objectField<ResultReference>({
    executionSessionId: stringField,
    resultId: stringField,
  }),
  (value) =>
    isUuid(value.executionSessionId) && /^[1-9]\d*$/.test(value.resultId)
      ? null
      : { fieldPath: "resultRef", reason: "invalid result reference" },
);

export const linearRegressionReportField = refineField(
  objectField<LinearRegressionReportData>({
    summary: refineField(
      objectField<LinearSummaryOptions>({
        equation: booleanField,
        model_summary: booleanField,
        anova: booleanField,
        coefficient_table: booleanField,
        coefficient_chart: booleanField,
        diagnostics: booleanField,
        residual_plot: booleanField,
        observations: booleanField,
        acf_pacf: booleanField,
        acf_max_lag: integerField,
        serial_tests: booleanField,
        serial_lags: integerField,
        bg_nomiss0: booleanField,
        hypothesis_test: booleanField,
        hypothesis: stringField,
      }),
      (value) =>
        value.acf_max_lag >= 1 &&
        value.acf_max_lag <= 40 &&
        value.serial_lags >= 1 &&
        value.serial_lags <= 40 &&
        (!value.hypothesis_test ||
          (value.hypothesis.trim().length > 0 &&
            new TextEncoder().encode(value.hypothesis).length <= 4096))
          ? null
          : { fieldPath: "summary", reason: "invalid summary parameters" },
    ),
    title: literalField("Linear Regression Summary"),
    endog_name: stringField,
    resultRef: resultReferenceField,
    paramNames: arrayField(stringField),
    presentation: objectField({
      summary: keyValueDataField,
      anova: tableDataField,
      conditionNumber: statCardDataField,
    }),
    coefficients: objectField({
      kind: literalField("tableRef"),
      part: literalField("coefficients"),
      rowCount: integerField,
    }),
    observations: objectField({
      kind: literalField("tableRef"),
      part: literalField("observations"),
      rowCount: integerField,
    }),
  }),
  (value) =>
    value.observations.rowCount ===
    value.presentation.summary.items.find((item) => item.id === "numObservations")?.value
      ? null
      : { fieldPath: "observations.rowCount", reason: "inconsistent observation count" },
);

export const linearCoefficientField = objectField<Coefficient>({
  category: optionalField(stringField),
  variable: stringField,
  coef: numberField,
  std_err: numberField,
  t_value: numberField,
  p_value: numberField,
  "confidence_interval_0.025": numberField,
  "confidence_interval_0.975": numberField,
  is_significant: booleanField,
});

const serialTestField = objectField({
  stat: numberField,
  p_value: numberField,
  lags: integerField,
});
const analysisFields = {
  residualPlot: refineField(
    objectField({
      points: arrayField(
        objectField({ observation: integerField, x: numberField, y: numberField }),
      ),
      totalCount: integerField,
      matchedCount: integerField,
      sampled: booleanField,
      sampling: literalField("systematic"),
    }),
    (value) =>
      value.matchedCount <= value.totalCount &&
      value.points.length <= value.matchedCount &&
      value.sampled === value.points.length < value.matchedCount &&
      value.points.every((point) => point.observation > 0 && point.observation <= value.totalCount)
        ? null
        : { fieldPath: "points", reason: "inconsistent plot population" },
  ),
  acfPacf: objectField({
    acf: arrayField(numberField),
    pacf: arrayField(numberField),
    n: integerField,
  }),
  serialTests: objectField({
    bg: optionalField(serialTestField),
    q: optionalField(serialTestField),
    dw: objectField({ d: numberField }),
  }),
  hypothesis: objectField({
    test_type: literalField("t", "wald"),
    h0_form: stringField,
    h1_form: stringField,
    alternative: stringField,
    r_beta_minus_r: numberField,
    stat: numberField,
    df1: numberField,
    df2: numberField,
    p_value: numberField,
  }),
};

function read<T>(field: ReportField<T>, value: unknown): T {
  const parsed = field.read(value, "$");
  if (!parsed.ok)
    throw new Error(`Invalid result analysis: ${parsed.issue.fieldPath} ${parsed.issue.reason}`);
  return parsed.value;
}

export function parseResultAnalysis(raw: unknown): ResultAnalysis {
  const envelope = read(
    objectField({
      kind: literalField("residualPlot", "acfPacf", "serialTests", "hypothesis"),
      value: { read: (value) => ({ ok: true as const, value }) },
    }),
    raw,
  );
  switch (envelope.kind) {
    case "residualPlot":
      return { kind: envelope.kind, value: read(analysisFields.residualPlot, envelope.value) };
    case "acfPacf":
      return { kind: envelope.kind, value: read(analysisFields.acfPacf, envelope.value) };
    case "serialTests":
      return { kind: envelope.kind, value: read(analysisFields.serialTests, envelope.value) };
    case "hypothesis":
      return { kind: envelope.kind, value: read(analysisFields.hypothesis, envelope.value) };
  }
}
