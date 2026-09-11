import type { LinearModelInfo } from "@/shared/types/report/regression";
import type { SerialTestsResponseDTO } from "@/shared/types/report/serialTests";
import type { ResultReference } from "./result";

export type ResultTablePart = "coefficients" | "observations";
export interface ResultTableReference<Part extends ResultTablePart = ResultTablePart> {
  readonly kind: "tableRef";
  readonly part: Part;
  readonly rowCount: number;
}

export interface OlsReportData {
  title: "OLS Summary";
  endog_name: string;
  resultRef: ResultReference;
  model_basic_info: LinearModelInfo;
  diagnostic_info: { cond_no: number };
  coefficients: ResultTableReference<"coefficients">;
  observations: ResultTableReference<"observations">;
}

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
  points: { observation: number; x: number; y: number }[];
  totalCount: number;
  matchedCount: number;
  sampled: boolean;
  sampling: "systematic";
}

export type ResultAnalysisRequest =
  | { kind: "residualPlot"; maxPoints: number; xRange?: [number, number] }
  | { kind: "acfPacf"; maxLag: number }
  | { kind: "serialTests"; lags: number; bgNomiss0: boolean }
  | { kind: "hypothesis"; hypothesis: string };

export type ResultAnalysisValues = {
  residualPlot: ResidualPlotResult;
  acfPacf: AcfPacfResult;
  serialTests: SerialTestsResponseDTO;
  hypothesis: HypothesisTestResult;
};

export type ResultAnalysis = {
  [Kind in keyof ResultAnalysisValues]: { kind: Kind; value: ResultAnalysisValues[Kind] };
}[keyof ResultAnalysisValues];
