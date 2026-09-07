import type { ResultDescriptor } from "@/shared/types/domain/result";
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
  if (parsed.ok) return { ok: true, value: parsed.value };

  return {
    ok: false,
    diagnostic: {
      resultId: descriptor.resultId,
      runId: descriptor.provenance.runId,
      nodeId: descriptor.provenance.nodeId,
      outputPinId: outputPinId(descriptor),
      presentation: { kind: "report", report },
      valueKind: descriptor.valueKind,
      fieldPath: parsed.issue.fieldPath,
      reason: parsed.issue.reason,
    },
  };
}
