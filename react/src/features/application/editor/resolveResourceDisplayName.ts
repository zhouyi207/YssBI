import { resourceKey, useResourceStore } from "@/features/core/resource";
import type { ResourceRef } from "@/features/core/resource/resourceTypes";

/** Display label for tabs / close-save prompts — ResourceStore is the source of truth. */
export function resolveResourceDisplayName(ref: ResourceRef | null, fallbackId = ""): string {
  if (!ref) return fallbackId || "Untitled";

  return useResourceStore.getState().resources[resourceKey(ref)]?.name ?? (fallbackId || ref.id);
}
