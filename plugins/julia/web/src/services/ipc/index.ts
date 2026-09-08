import { request } from "@/sdk";
export interface IpcError {
  code: string;
  details: Record<string, unknown> | null;
  incidentId: string | null;
}
export function normalizeIpcError(_command: string, error: unknown): IpcError {
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
function task(value: unknown) {
  const state = value as { taskId: string; state: string; error: IpcError | null };
  const statuses: Record<string, string> = {
    admitted: "queued",
    running: "running",
    cancelRequested: "cancelling",
    succeeded: "completed",
    failed: "failed",
    cancelled: "cancelled",
    outcomeUnknown: "failed",
  };
  return {
    taskId: state.taskId,
    status: statuses[state.state] ?? "failed",
    progress: null,
    error: state.error,
  };
}
export async function invokeCommand<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  try {
    if (command === "submit_bayes_inference")
      return task(
        await request("tasks.start", {
          taskType: "bayes.inference",
          operationId: `op-${crypto.randomUUID()}`,
          parameters: args.input,
          timeoutMs: 3_600_000,
        }),
      ) as T;
    if (command === "get_bayes_inference_status")
      return task(await request("tasks.get", args)) as T;
    if (command === "cancel_bayes_inference") return await request<T>("tasks.cancel", args);
    if (command === "read_bayes_inference_result") return await request<T>("tasks.result", args);
    return await request<T>("commands.execute", { commandId: command, args });
  } catch (error) {
    throw normalizeIpcError(command, error);
  }
}
