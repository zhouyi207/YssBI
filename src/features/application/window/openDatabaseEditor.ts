import { createPersistedWindow } from "./createPersistedWindow";
import { createEphemeralWindowLabel } from "./windowLabels";
import { logger } from "@/features/application/observability/appLogger";
import { normalizeApplicationIpcError } from "@/features/application/errorReference";
import { i18n } from "@/app/i18n";

export async function openDatabaseEditorWindow(databaseId?: string): Promise<void> {
  try {
    const label = createEphemeralWindowLabel("dataview");
    const url = databaseId
      ? `index.html?database=${encodeURIComponent(databaseId)}#/database`
      : "index.html#/database";
    await createPersistedWindow({
      geometry: { source: "backend", kind: "databaseEditor" },
      label,
      url,
      title: i18n.t("databaseEditor.title"),
    });
  } catch (error) {
    const ipcError = normalizeApplicationIpcError("open_database_editor_window", error);
    logger.app.error(
      `Failed to open data view code=${ipcError.code} incidentId=${ipcError.incidentId ?? "none"}`,
      "Window",
    );
    throw error;
  }
}
