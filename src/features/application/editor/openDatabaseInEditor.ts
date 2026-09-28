import { openEditorPanel, isEditorOpenRejectionHandled } from "./openEditorPanel";
import { revealActiveEditorDetails } from "./editorPanelActivation";

export async function openDatabaseInEditor(databaseId: string): Promise<void> {
  try {
    const panel = await openEditorPanel({
      resourceRef: databaseId,
      resourceKind: "database",
    });
    await revealActiveEditorDetails(panel);
  } catch (error) {
    if (!isEditorOpenRejectionHandled(error)) throw error;
  }
}
