import type {
  DiagnosticWarningDTO,
  InferenceResultDTO,
  ParameterSummaryDTO,
} from "@/shared/types/bayes";

export type DiagnosticSeverity = "good" | "warning" | "bad" | "unknown";
export type ParameterDiagnosticStatus = "ok" | "check_rhat" | "low_ess" | "unknown";

export type DiagnosticSuggestion =
  | "check_metrics"
  | "save_samples"
  | "increase_sampling"
  | "inspect_plots";

export interface DiagnosticAssessment {
  severity: DiagnosticSeverity;
  suggestions: DiagnosticSuggestion[];
  metrics: DiagnosticMetric[];
  warnings: DiagnosticWarningDescription[];
}

export interface DiagnosticMetric {
  key: "sampling" | "rhat" | "ess" | "divergences" | "max_treedepth_hits";
  severity: DiagnosticSeverity;
}

export interface DiagnosticWarningDescription {
  code: string;
  metric: DiagnosticWarningDTO["metric"];
  value: number;
  threshold: number;
  parameter: string;
}

const RHAT_WARNING_THRESHOLD = 1.01;
const RHAT_BAD_THRESHOLD = 1.1;
const MIN_ESS = 100;

export function evaluateInferenceDiagnostics(
  result: InferenceResultDTO | null,
): DiagnosticAssessment {
  if (!result) {
    return {
      severity: "unknown",
      suggestions: [],
      metrics: [],
      warnings: [],
    };
  }

  const summaries = result.summaries;
  const warnings = result.diagnostics.warnings ?? [];
  const missingDiagnostics = summaries.some(
    (summary) => summary.rhat == null || summary.essBulk == null || summary.essTail == null,
  );
  const hasBadRhat = summaries.some((summary) => (summary.rhat ?? 0) > RHAT_BAD_THRESHOLD);
  const hasWarningRhat = summaries.some((summary) => (summary.rhat ?? 0) > RHAT_WARNING_THRESHOLD);
  const hasLowEss = summaries.some(
    (summary) => isLowEss(summary.essBulk) || isLowEss(summary.essTail),
  );
  const hasDivergences = (result.diagnostics.divergences ?? 0) > 0;
  const hasTreedepthHits = (result.diagnostics.maxTreedepthHits ?? 0) > 0;
  const hasBackendWarning = warnings.length > 0;
  const details = diagnosticDetails(result, {
    hasBadRhat,
    hasWarningRhat,
    hasLowEss,
    missingDiagnostics,
  });

  if (hasBadRhat || hasDivergences) {
    return {
      severity: "bad",
      suggestions: convergenceSuggestions(),
      ...details,
    };
  }
  if (hasWarningRhat || hasLowEss || hasTreedepthHits || hasBackendWarning) {
    return {
      severity: "warning",
      suggestions: convergenceSuggestions(),
      ...details,
    };
  }
  if (missingDiagnostics) {
    return {
      severity: "unknown",
      suggestions: ["check_metrics", "save_samples"],
      ...details,
    };
  }
  return {
    severity: "good",
    suggestions: [],
    ...details,
  };
}

export function parameterDiagnosticStatus(summary: ParameterSummaryDTO): ParameterDiagnosticStatus {
  if (summary.rhat == null || summary.essBulk == null || summary.essTail == null) return "unknown";
  if (summary.rhat > RHAT_WARNING_THRESHOLD) return "check_rhat";
  if (isLowEss(summary.essBulk) || isLowEss(summary.essTail)) return "low_ess";
  return "ok";
}

export function describeDiagnosticWarning(
  warning: DiagnosticWarningDTO,
): DiagnosticWarningDescription {
  return {
    code: warning.code,
    metric: warning.metric,
    value: warning.value,
    threshold: warning.threshold,
    parameter: warning.parameter,
  };
}

export function diagnosticSeverityClass(severity: DiagnosticSeverity): string {
  switch (severity) {
    case "good":
      return "text-emerald-500";
    case "warning":
      return "text-amber-500";
    case "bad":
      return "text-destructive";
    case "unknown":
      return "text-muted-foreground";
  }
}

function diagnosticDetails(
  result: InferenceResultDTO,
  flags: {
    hasBadRhat: boolean;
    hasWarningRhat: boolean;
    hasLowEss: boolean;
    missingDiagnostics: boolean;
  },
): Pick<DiagnosticAssessment, "metrics" | "warnings"> {
  const diagnostics = result.diagnostics;
  const rhatSeverity: DiagnosticSeverity = flags.missingDiagnostics
    ? "unknown"
    : flags.hasBadRhat
      ? "bad"
      : flags.hasWarningRhat
        ? "warning"
        : "good";
  const essSeverity: DiagnosticSeverity = flags.missingDiagnostics
    ? "unknown"
    : flags.hasLowEss
      ? "warning"
      : "good";
  return {
    metrics: [
      { key: "sampling", severity: "good" },
      { key: "rhat", severity: rhatSeverity },
      { key: "ess", severity: essSeverity },
      { key: "divergences", severity: (diagnostics.divergences ?? 0) > 0 ? "bad" : "good" },
      {
        key: "max_treedepth_hits",
        severity:
          diagnostics.maxTreedepthHits == null
            ? "unknown"
            : diagnostics.maxTreedepthHits > 0
              ? "warning"
              : "good",
      },
    ],
    warnings: (diagnostics.warnings ?? []).map(describeDiagnosticWarning),
  };
}

function isLowEss(value: number | null | undefined): boolean {
  return value != null && value < MIN_ESS;
}

function convergenceSuggestions(): DiagnosticSuggestion[] {
  return ["increase_sampling", "inspect_plots"];
}
