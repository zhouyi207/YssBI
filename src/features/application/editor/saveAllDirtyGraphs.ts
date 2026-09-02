import i18n from "i18next";

import { saveChartDocument } from "@/features/application/chart/saveChartDocument";
import { workbenchDockviewRead } from "@/modules/workbench/public";
import { isResourceDocumentDirty, resourceKey } from "@/features/core/resource";
import { logger } from "@/features/application/observability/appLogger";

import { showBlockingIpcError, showBlockingMessage } from "./blockingErrorDialog";
import { resolveResourceDisplayName } from "./resolveResourceDisplayName";
import { saveGraphDraft } from "@/features/application/graphDraft/saveGraphDraft";

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

      const saved = await saveGraphDraft(document.resourceRef, document.resourceKind);
      if (!saved) return false;
    } catch (error) {
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
