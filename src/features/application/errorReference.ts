import {
  IPC_ERROR_BRAND,
  IPC_MALFORMED_ERROR_CODE,
  IPC_TRANSPORT_FAILURE_CODE,
} from "@/shared/constants/ipcError";
export type { ErrorReference } from "@/shared/types/domain/diagnostics";
import type { ErrorReference } from "@/shared/types/domain/diagnostics";

export interface ApplicationIpcError extends ErrorReference {
  readonly details: unknown;
}

/** Detects the branded error produced by the Service IPC adapter without importing that adapter. */
export function isApplicationIpcError(error: unknown): error is ApplicationIpcError {
  if (typeof error !== "object" || error === null) return false;
  const candidate = error as {
    readonly [IPC_ERROR_BRAND]?: unknown;
    readonly code?: unknown;
    readonly incidentId?: unknown;
    readonly details?: unknown;
  };
  return (
    candidate[IPC_ERROR_BRAND] === true &&
    typeof candidate.code === "string" &&
    (candidate.incidentId === null || typeof candidate.incidentId === "string") &&
    "details" in candidate
  );
}

/** Normalizes a Service boundary failure for Application presentation and recovery decisions. */
export function normalizeApplicationIpcError(
  _command: string,
  error: unknown,
): ApplicationIpcError {
  if (isApplicationIpcError(error)) return error;
  if (error instanceof Error) {
    return {
      code: IPC_TRANSPORT_FAILURE_CODE,
      details: null,
      incidentId: null,
    };
  }
  return {
    code: IPC_MALFORMED_ERROR_CODE,
    details: null,
    incidentId: null,
  };
}

export function isApplicationIpcErrorCode<Code extends string>(
  value: unknown,
  code: Code,
): value is ApplicationIpcError & { readonly code: Code } {
  return isApplicationIpcError(value) && value.code === code;
}

/** Converts an untrusted boundary failure into the Application error projection. */
export function toErrorReference(error: unknown, fallbackCode: string): ErrorReference {
  if (typeof error !== "object" || error === null) {
    return { code: fallbackCode, incidentId: null };
  }
  const candidate = error as { code?: unknown; incidentId?: unknown };
  return typeof candidate.code === "string"
    ? {
        code: candidate.code,
        incidentId: typeof candidate.incidentId === "string" ? candidate.incidentId : null,
      }
    : { code: fallbackCode, incidentId: null };
}
