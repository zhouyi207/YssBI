import { request } from "@/sdk";
export interface IpcError {
  code: string;
  details: Record<string, unknown> | null;
  incidentId: string | null;
}
export function normalizeIpcError(error: unknown): IpcError {
  if (error && typeof error === "object" && "code" in error && typeof error.code === "string")
    return {
      code: error.code,
      details: (error as IpcError).details ?? null,
      incidentId: (error as IpcError).incidentId ?? null,
    };
  return { code: "plugin_request_failed", details: null, incidentId: null };
}
export function isIpcError(value: unknown): value is IpcError {
  return !!value && typeof value === "object" && "code" in value;
}
export async function invokeCommand<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  try {
    return await request<T>("commands.execute", { commandId: command, args });
  } catch (error) {
    throw normalizeIpcError(error);
  }
}
