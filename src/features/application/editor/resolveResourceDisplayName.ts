import { lookupGraphResource, useResourceStore } from "@/features/core/resource";
import type { ResourceRef } from "@/features/core/resource/resourceTypes";
import { useWorksheetStore } from "@/features/core/worksheet/worksheetStore";

/** Display label for tabs / close-save prompts — ResourceStore is the source of truth. */
export function resolveResourceDisplayName(ref: ResourceRef | null, fallbackId = ""): string {
  if (!ref) return fallbackId || "Untitled";

  if (ref.kind === "event" || ref.kind === "function") {
    const meta = lookupGraphResource(useResourceStore.getState().resources, ref.id, ref.kind);
    return meta?.name ?? fallbackId ?? ref.id;
  }

  if (ref.kind === "worksheet") {
    const indexEntry = useWorksheetStore
      .getState()
      .index.find((worksheet) => worksheet.worksheetPath === ref.id);
    return indexEntry?.name ?? fallbackId ?? ref.id;
  }

  return fallbackId ?? ref.id;
}
