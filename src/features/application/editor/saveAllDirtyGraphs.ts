import i18n from "i18next";

import { warnCallFunctionIssuesBeforeSave } from "@/features/application/graphDiagnostics/warnCallFunctionIssues";
import { saveChartDocument } from "@/features/application/chart/saveChartDocument";
import {
  captureSettledGraphSaveCommandContext,
  isGraphSaveCommandRevisionCurrent,
  type GraphSaveCommandContext,
} from "@/features/application/projectCommandContext";
import { workbenchDockviewRead } from "@/features/core/dockview/workbenchRead";
import { isResourceDocumentDirty, markResourceDirty, resourceKey } from "@/features/core/resource";
import { GraphService } from "@/services/graph/graphService";
import { logger } from "@/features/application/observability/appLogger";

import { showBlockingIpcError, showBlockingMessage } from "./blockingErrorDialog";
import { resolveResourceDisplayName } from "./resolveResourceDisplayName";

interface DirtyEditorDocument {
  resourceRef: string;
  title: string;
  resourceKind: "event" | "function" | "chart";
}

function collectDirtyEditorDocuments(): DirtyEditorDocument[] {
  const seen = new Set<string>();
  const dirty: DirtyEditorDocument[] = [];
  for (const panel of workbenchDockviewRead.listPanels()) {
    if (panel.metadata.role !== "editor") continue;
    const { resourceRef, resourceKind } = panel.metadata;
    const key = resourceKey({ id: resourceRef, kind: resourceKind });
    if (seen.has(key) || !isResourceDocumentDirty({ id: resourceRef, kind: resourceKind })) {
      continue;
    }
    seen.add(key);
    dirty.push({
      resourceRef,
      resourceKind,
      title: resolveResourceDisplayName(
        { id: resourceRef, kind: resourceKind },
        panel.title ?? resourceRef,
      ),
    });
  }
  return dirty;
}

/** Persist every dirty document currently projected by canonical editor metadata. */
export async function saveAllDirtyGraphs(): Promise<boolean> {
  const dirty = collectDirtyEditorDocuments();
  if (dirty.length === 0) return true;

  for (const document of dirty) {
    let context: GraphSaveCommandContext | undefined;
    try {
      if (document.resourceKind === "chart") {
        const saved = await saveChartDocument(document.resourceRef);
        if (!saved) {
          showBlockingMessage(
            i18n.t("notifications.editor.documentSaveFailed", {
              title: document.title,
              error: "chart_save_not_committed",
            }),
          );
          return false;
        }
        continue;
      }

      warnCallFunctionIssuesBeforeSave(document.resourceRef);
      context = await captureSettledGraphSaveCommandContext(document.resourceRef);
      await GraphService.saveProjectGraph(
        context.projectInstanceId,
        document.resourceRef,
        context.expectedRevision,
        context.operationId,
      );
      if (!isGraphSaveCommandRevisionCurrent(context, document.resourceRef)) return false;
      markResourceDirty(
        {
          id: document.resourceRef,
          kind: document.resourceKind,
        },
        false,
      );
    } catch (error) {
      if (context && !context.isCurrent()) return false;
      const message = error instanceof Error ? error.message : String(error);
      logger.app.error(
        `Failed to save graph '${document.title}' (${document.resourceRef}): ${message}`,
        "saveAllDirtyGraphs",
      );
      showBlockingIpcError(
        error,
        document.resourceKind === "chart" ? "save_chart" : "save_project_graph",
        (code) =>
          i18n.t("notifications.editor.documentSaveFailed", {
            title: document.title,
            error: code,
          }),
      );
      return false;
    }
  }
  return true;
}
