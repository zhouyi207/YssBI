import { logger } from "./appLogger";

export type ViewDiagnosticDomain = "app" | "data";

/** Application-owned reporting action for the few View-level presentation failures. */
export function reportViewIssue(
  domain: ViewDiagnosticDomain,
  error: unknown,
  source: string,
): void {
  logger[domain].error(String(error), source);
}
