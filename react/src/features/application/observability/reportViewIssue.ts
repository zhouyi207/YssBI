import { logger } from "@/utils/frontendLogger";
import type { ReportValidationDiagnostic } from "@/shared/types/report/reportValidation";
import { formatApplicationIpcError } from "@/features/application/errorReference";

export type ViewDiagnosticDomain = "app" | "data";

/** Application-owned reporting action for the few View-level presentation failures. */
export function reportViewIssue(
  domain: ViewDiagnosticDomain,
  error: unknown,
  source: string,
): void {
  logger[domain].error(formatApplicationIpcError(error), source);
}

/** The report parser owns the diagnostic; observations retain only its validated identity. */
export function reportInvalidReport(diagnostic: ReportValidationDiagnostic): void {
  logger.data.error(
    `report_validation_failed resultId=${diagnostic.resultId} runId=${diagnostic.runId} nodeId=${diagnostic.nodeId} report=${diagnostic.presentation.report} valueKind=${diagnostic.valueKind}`,
    "ReportValidation",
  );
}
