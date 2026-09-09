import { openEditorPanel, isEditorOpenRejectionHandled } from "./openEditorPanel";
import { activateEditorPanelAndSyncSession } from "./activateEditorPanelAndSyncSession";

export async function openDatabaseInEditor(databaseId: string): Promise<void> {
  try {
    const panel = await openEditorPanel({
      resourceRef: databaseId,
      resourceKind: "database",
      pinned: true,
    });
    await activateEditorPanelAndSyncSession(panel);
  } catch (error) {
    if (!isEditorOpenRejectionHandled(error)) throw error;
  }
}
