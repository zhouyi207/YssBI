import { invoke, type InvokeArgs, type InvokeOptions } from "@tauri-apps/api/core";
import { normalizeIpcError } from "./ipcError";

export async function invokeCommand<Result>(
  command: string,
  args?: InvokeArgs,
  options?: InvokeOptions,
): Promise<Result> {
  try {
    if (options !== undefined) return await invoke<Result>(command, args, options);
    if (args !== undefined) return await invoke<Result>(command, args);
    return await invoke<Result>(command);
  } catch (error) {
    throw normalizeIpcError(command, error);
  }
}
