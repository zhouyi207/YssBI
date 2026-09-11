import type { OlsReportData, ResultAnalysis } from "@/shared/types/domain/resultReport";
import type { ResultReference } from "@/shared/types/domain/result";
import type { Coefficient } from "@/shared/types/report/regression";
import { linearModelInfoField } from "@/shared/types/report/parseCommon";
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

export const olsReportField = refineField(
  objectField<OlsReportData>({
    title: literalField("OLS Summary"),
    endog_name: stringField,
    resultRef: resultReferenceField,
    model_basic_info: linearModelInfoField,
    diagnostic_info: objectField({ cond_no: numberField }),
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
    value.observations.rowCount === value.model_basic_info.num_observation
      ? null
      : { fieldPath: "observations.rowCount", reason: "inconsistent observation count" },
);

export const olsCoefficientField = objectField<Coefficient>({
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
