import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import type { PlatformFailure, PlatformOutcome } from "./platformTypes";

function targetFailure(operation: "openExternal" | "revealPath"): PlatformFailure {
  return { operation, code: "invalidArgument", argument: "target" };
}

function operationFailure(operation: "openExternal" | "revealPath"): PlatformFailure {
  return { operation, code: "operationFailed" };
}

export async function openExternal(target: string): Promise<PlatformOutcome<void>> {
  if (target.trim().length === 0) return { ok: false, failure: targetFailure("openExternal") };
  try {
    await openUrl(target);
    return { ok: true, value: undefined };
  } catch {
    return { ok: false, failure: operationFailure("openExternal") };
  }
}

export async function revealPath(target: string): Promise<PlatformOutcome<void>> {
  if (target.trim().length === 0) return { ok: false, failure: targetFailure("revealPath") };
  try {
    await revealItemInDir(target);
    return { ok: true, value: undefined };
  } catch {
    return { ok: false, failure: operationFailure("revealPath") };
  }
}
