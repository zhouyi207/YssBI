import type { FileResourceKind } from "@/shared/types/domain/resource";
import { resourceKey, useResourceStore } from "@/features/core/resource";
import { openGraphInEditor } from "./openGraphInEditor";
import { isEditorOpenRejectionHandled, openEditorPanel } from "./openEditorPanel";
import { revealActiveEditorDetails } from "./editorPanelActivation";
import {
  captureProjectLifecycleState,
  isProjectLifecycleStateCurrent,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";

type OpenFileOptions = { targetGroupId?: string };
function fileDisplayName(path: string, kind: FileResourceKind): string {
  return useResourceStore.getState().resources[resourceKey({ id: path, kind })]?.name ?? path;
}
async function openPanel(
  path: string,
  kind: FileResourceKind,
  options?: OpenFileOptions,
): Promise<void> {
  const identity = captureProjectLifecycleState();
  const panel = await openEditorPanel({ resourceRef: path, resourceKind: kind }, options);
  if (!isProjectLifecycleStateCurrent(identity)) return;
  await revealActiveEditorDetails(panel);
}
const openers: Record<
  FileResourceKind,
  (path: string, options?: OpenFileOptions) => Promise<void>
> = {
  event_graph: async (path, options) => {
    await openGraphInEditor(
      path,
      fileDisplayName(path, "event_graph"),
      "event_graph",
      options?.targetGroupId,
    );
  },
  function_graph: async (path, options) => {
    await openGraphInEditor(
      path,
      fileDisplayName(path, "function_graph"),
      "function_graph",
      options?.targetGroupId,
    );
  },
  chart: (path, options) => openPanel(path, "chart", options),
  mind: (path, options) => openPanel(path, "mind", options),
  doc: (path, options) => openPanel(path, "doc", options),
};
export async function openFileInEditor(
  path: string,
  kind: FileResourceKind,
  options?: OpenFileOptions,
): Promise<void> {
  try {
    await openers[kind](path, options);
  } catch (error) {
    if (!isEditorOpenRejectionHandled(error)) throw error;
  }
}
