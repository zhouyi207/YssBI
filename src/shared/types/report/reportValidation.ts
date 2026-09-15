import { isResultReference, type ResultDescriptor } from "@/shared/types/domain/result";
import { isRecord } from "./guards";
import { parseReportPayloadResult } from "./parseReportPayload";
import type { ReportPayloadKind } from "./reportKinds";

export interface ReportValidationDiagnostic {
  resultId: string;
  runId: string;
  nodeId: string;
  outputPinId: string | null;
  presentation: { kind: "report"; report: ReportPayloadKind };
  valueKind: ResultDescriptor["valueKind"];
  fieldPath: string;
  reason: string;
}

export type ReportValidationResult =
  | { ok: true; value: unknown }
  | { ok: false; diagnostic: ReportValidationDiagnostic };

function outputPinId(descriptor: ResultDescriptor): string | null {
  const port = descriptor.provenance.output?.port;
  if (!port) return null;
  return port.kind === "declared" ? port.portKey : `${port.templateKey}/${port.instanceId}`;
}

export function validateReportPayload(
  descriptor: ResultDescriptor,
  report: ReportPayloadKind,
  raw: unknown,
): ReportValidationResult {
  const parsed = parseReportPayloadResult(report, raw);
  const identityMatches =
    report !== "olsSummary" ||
    (parsed.ok &&
      isRecord(parsed.value) &&
      isResultReference(parsed.value.resultRef) &&
      parsed.value.resultRef.executionSessionId === descriptor.executionSessionId &&
      parsed.value.resultRef.resultId === descriptor.resultId);
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
