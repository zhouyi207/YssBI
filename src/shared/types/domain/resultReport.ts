import type { UiKeyValueData, UiTableData, UiStatCardData } from "./uiData";
import type { SerialTestsResponseDTO } from "@/shared/types/report/serialTests";
import type { ResultReference } from "./result";

export type StructuredResultPart = `structured:/${string}`;
export type ResultTablePart = "coefficients" | "observations" | StructuredResultPart;
export interface ResultTableReference<Part extends ResultTablePart = ResultTablePart> {
  readonly kind: "tableRef";
  readonly part: Part;
  readonly rowCount: number;
}

export function isStructuredTableReference(
  value: unknown,
): value is ResultTableReference<StructuredResultPart> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const reference = value as Record<string, unknown>;
  return (
    Object.keys(reference).length === 3 &&
    reference.kind === "tableRef" &&
    typeof reference.part === "string" &&
    reference.part.startsWith("structured:/") &&
    reference.part.length <= 4107 &&
    !/~(?:[^01]|$)/.test(reference.part) &&
    reference.part.slice("structured:/".length).split("/").length <= 64 &&
    typeof reference.rowCount === "number" &&
    Number.isSafeInteger(reference.rowCount) &&
    reference.rowCount >= 0
  );
}

export interface LinearRegressionReportData {
  summary: LinearSummaryOptions;
  title: "Linear Regression Summary";
  endog_name: string;
  resultRef: ResultReference;
  paramNames: string[];
  presentation: {
    readonly summary: UiKeyValueData;
    readonly anova: UiTableData;
    readonly conditionNumber: UiStatCardData;
  };
  coefficients: ResultTableReference<"coefficients">;
  observations: ResultTableReference<"observations">;
}

export interface LinearSummaryOptions {
  equation: boolean;
  model_summary: boolean;
  anova: boolean;
  coefficient_table: boolean;
  coefficient_chart: boolean;
  diagnostics: boolean;
  residual_plot: boolean;
  observations: boolean;
  acf_pacf: boolean;
  acf_max_lag: number;
  serial_tests: boolean;
  serial_lags: number;
  bg_nomiss0: boolean;
  hypothesis_test: boolean;
  hypothesis: string;
}

export const LINEAR_SUMMARY_CONTENTS = [
  { key: "model_summary", section: "modelSummary" },
  { key: "coefficient_table", section: "coefficientTable" },
  { key: "coefficient_chart", section: "coefficientMagnitude" },
  { key: "equation", section: "equation" },
  { key: "anova", section: "anova" },
  { key: "diagnostics", section: "diagnostics" },
  { key: "residual_plot", section: "residualPlot" },
  { key: "observations", section: "observations" },
  { key: "acf_pacf", section: "acfPacf" },
  { key: "serial_tests", section: "serialTests" },
  { key: "hypothesis_test", section: "hypothesisTest" },
] as const;

export type LinearSummaryContent = (typeof LINEAR_SUMMARY_CONTENTS)[number]["key"];
export type LinearSummaryAddition = Partial<Record<LinearSummaryContent, true>> &
  Partial<Pick<LinearSummaryOptions, "acf_max_lag" | "serial_lags" | "bg_nomiss0" | "hypothesis">>;

export interface AcfPacfResult {
  acf: number[];
  pacf: number[];
  n: number;
}

export interface HypothesisTestResult {
  test_type: "t" | "wald";
  h0_form: string;
  h1_form: string;
  alternative: string;
  r_beta_minus_r: number;
  stat: number;
  df1: number;
  df2: number;
  p_value: number;
}

export interface ResidualPlotResult {
  points: { observation: number; x: number; y: number; highlighted: boolean }[];
  totalCount: number;
  matchedCount: number;
  sampled: boolean;
  sampling: "systematic";
  highlightAvailable: boolean;
}

export type ResultAnalysisRequest =
  | {
      kind: "residualPlot";
      maxPoints: number;
      xRange?: [number, number];
      adjacent: boolean;
      highlightTopPercent?: number;
    }
  | { kind: "diagnostics" }
  | { kind: "acfPacf" }
  | { kind: "serialTests" }
  | { kind: "hypothesis" };

export type ResultAnalysisValues = {
  residualPlot: ResidualPlotResult;
  diagnostics: {
    tests: { name: string; value: unknown; unavailable_reason: string | null }[];
    leverage_density: { x: number; y: number }[];
    leverage_unavailable_reason: string | null;
  };
  acfPacf: AcfPacfResult;
  serialTests: SerialTestsResponseDTO;
  hypothesis: HypothesisTestResult;
};

export type ResultAnalysis = {
  [Kind in keyof ResultAnalysisValues]: { kind: Kind; value: ResultAnalysisValues[Kind] };
}[keyof ResultAnalysisValues];
