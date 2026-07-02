import { PIN_COLORS } from "@/features/domain/sidebar";
import type { DataType } from "@/shared/types/domain/dataType";
import { dataTypeDisplay } from "@/shared/types/domain/dataType";
import { createPersistedWindow } from "@/features/application/window";
import { uiStore } from "@/features/core/ui/UIStore";
import { logger } from "@/utils/appLogger";
import { i18n } from "@/app/i18n";

export async function openDataViewWindow(databaseId?: string): Promise<void> {
  try {
    const label = `dataview-${Math.random().toString(36).substring(7)}`;
    const url = databaseId
      ? `index.html?database=${encodeURIComponent(databaseId)}#/dataview`
      : "index.html#/dataview";
    await createPersistedWindow({
      kind: "dataView",
      label,
      url,
      title: i18n.t("dataView.title"),
    });
  } catch (error) {
    logger.app.error(`Failed to open data view: ${error instanceof Error ? error.message : String(error)}`, "Sidebar");
    uiStore.showToast(i18n.t("dataView.failedOpenWindow"), "error");
  }
}

export function safeDataTypeDisplay(dataType: unknown): string {
  if (typeof dataType === "string") return dataType;
  if (dataType && typeof dataType === "object" && "kind" in dataType) {
    return dataTypeDisplay(dataType as DataType);
  }
  return "";
}

export function safeDataTypeColor(dataType: unknown): string {
  if (typeof dataType === "string") return PIN_COLORS[dataType] ?? "rgba(156,163,175,0.7)";
  if (dataType && typeof dataType === "object" && "kind" in dataType) {
    return PIN_COLORS[(dataType as DataType).kind] ?? "rgba(156,163,175,0.7)";
  }
  return "rgba(156,163,175,0.7)";
}
