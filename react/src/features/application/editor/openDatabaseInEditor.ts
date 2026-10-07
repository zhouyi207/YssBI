import { openEditorPanel, isEditorOpenRejectionHandled } from "./openEditorPanel";
import { revealActiveEditorDetails } from "./editorPanelActivation";
import {
  captureProjectLifecycleState,
  isProjectLifecycleStateCurrent,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";

export async function openDatabaseInEditor(databaseId: string): Promise<void> {
  const identity = captureProjectLifecycleState();
  try {
    const panel = await openEditorPanel({
      resourceRef: databaseId,
      resourceKind: "database",
    });
    if (!isProjectLifecycleStateCurrent(identity)) return;
    await revealActiveEditorDetails(panel);
  } catch (error) {
    if (!isProjectLifecycleStateCurrent(identity)) return;
    if (!isEditorOpenRejectionHandled(error)) throw error;
  }
}
