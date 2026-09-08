import { isIpcError, normalizeIpcError } from "@/services/ipc";
export interface ErrorReference {
  code: string;
  incidentId: string | null;
}
export const isApplicationIpcError = isIpcError;
export function toErrorReference(value: unknown, code: string): ErrorReference {
  return isIpcError(value) ? normalizeIpcError("plugin", value) : { code, incidentId: null };
}
