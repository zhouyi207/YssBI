import type { FileResourceKind } from "@/shared/types/domain/resource";
import { resourceKey, useResourceStore } from "@/features/core/resource";
import { useChartDocumentStore } from "@/features/core/chart/chartDocumentStore";
import { ChartService } from "@/services/chart/chartService";
import { mindActions } from "@/features/application/resource/mindActions";
import { docActions } from "@/features/application/resource/docActions";
import { captureProjectCommandContext } from "@/features/application/projectCommandContext";
import { openGraphInEditor } from "./openGraphInEditor";
import { isEditorOpenRejectionHandled, openEditorPanel } from "./openEditorPanel";
import { activateEditorPanelAndSyncSession } from "./activateEditorPanelAndSyncSession";

type OpenFileOptions = { targetGroupId?: string };
function fileDisplayName(path: string, kind: FileResourceKind): string {
  return useResourceStore.getState().resources[resourceKey({ id: path, kind })]?.name ?? path;
}
async function openPanel(
  path: string,
  kind: FileResourceKind,
  options?: OpenFileOptions,
): Promise<void> {
  const panel = await openEditorPanel({ resourceRef: path, resourceKind: kind }, options);
  await activateEditorPanelAndSyncSession(panel);
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
  chart: async (path, options) => {
    const context = captureProjectCommandContext();
    if (!useChartDocumentStore.getState().documents[path]) {
      const document = await ChartService.loadChart(context.projectInstanceId, path);
      context.assertCurrent();
      useChartDocumentStore.getState().upsertDocument(path, document);
    }
    context.assertCurrent();
    await openPanel(path, "chart", options);
  },
  mind: async (path, options) => {
    await mindActions.load(path);
    await openPanel(path, "mind", options);
  },
  doc: async (path, options) => {
    await docActions.load(path);
    await openPanel(path, "doc", options);
  },
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
