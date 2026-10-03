import type { ResultDescriptor, ResultReportKind } from "@/shared/types/domain/result";
import { parseReportPayloadResult, type ParsedReportPayload } from "./parseReportPayload";

export interface ReportValidationDiagnostic {
  resultId: string;
  runId: string;
  nodeId: string;
  outputPinId: string | null;
  presentation: { kind: "report"; report: ResultReportKind };
  valueKind: ResultDescriptor["valueKind"];
  fieldPath: string;
  reason: string;
}

export type ReportValidationResult =
  | { ok: true; value: ParsedReportPayload }
  | { ok: false; diagnostic: ReportValidationDiagnostic };

function outputPinId(descriptor: ResultDescriptor): string | null {
  const port = descriptor.provenance.output?.port;
  if (!port) return null;
  return port.kind === "declared" ? port.portKey : `${port.templateKey}/${port.instanceId}`;
}

export function validateReportPayload(
  descriptor: ResultDescriptor,
  report: ResultReportKind,
  raw: unknown,
): ReportValidationResult {
  const parsed = parseReportPayloadResult(report, raw);
  const identityMatches =
    report !== "linearRegressionSummary" ||
    (parsed.ok &&
      parsed.value.kind === "linearRegressionSummary" &&
      parsed.value.data.resultRef.executionSessionId === descriptor.executionSessionId &&
      parsed.value.data.resultRef.resultId === descriptor.resultId);
  if (parsed.ok && identityMatches) return { ok: true, value: parsed.value };

  return {
    ok: false,
    diagnostic: {
      resultId: descriptor.resultId,
      runId: descriptor.provenance.runId,
      nodeId: descriptor.provenance.nodeId,
      outputPinId: outputPinId(descriptor),
      presentation: { kind: "report", report },
      valueKind: descriptor.valueKind,
      fieldPath: parsed.ok ? "resultRef" : parsed.issue.fieldPath,
      reason: parsed.ok ? "does not match the report result" : parsed.issue.reason,
    },
  };
}
