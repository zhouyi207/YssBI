import { parseSettingsPatch } from "@/shared/types/settings/parseSettings";
import { emit, listen } from "@tauri-apps/api/event";
import type { PartialAppSettings } from "@/shared/types/settings";
import type { PlatformFailure, PlatformOutcome, PlatformUnsubscribe } from "./platformTypes";

export const SETTINGS_CHANGED_EVENT = "client-settings-updated";

function operationFailure(
  operation: "publishSettingsChanged" | "subscribeSettingsChanged",
): PlatformFailure {
  return { operation, code: "operationFailed" };
}

function invalidPayload(): PlatformFailure {
  return {
    operation: "subscribeSettingsChanged",
    code: "invalidResult",
    resultKind: "eventPayload",
  };
}

export async function publishSettingsChanged(
  settings: PartialAppSettings,
): Promise<PlatformOutcome<void>> {
  try {
    await emit(SETTINGS_CHANGED_EVENT, settings);
    return { ok: true, value: undefined };
  } catch {
    return { ok: false, failure: operationFailure("publishSettingsChanged") };
  }
}

export async function subscribeSettingsChanged(
  listener: (outcome: PlatformOutcome<PartialAppSettings>) => void,
): Promise<PlatformOutcome<PlatformUnsubscribe>> {
  try {
    const unlisten = await listen<unknown>(SETTINGS_CHANGED_EVENT, (event) => {
      const patch = parseSettingsPatch(event.payload);
      listener(patch ? { ok: true, value: patch } : { ok: false, failure: invalidPayload() });
    });
    return { ok: true, value: unlisten };
  } catch {
    return { ok: false, failure: operationFailure("subscribeSettingsChanged") };
  }
}
