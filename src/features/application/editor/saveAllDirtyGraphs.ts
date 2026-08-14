import { GraphService } from '@/services/graph/graphService';
import { editorDockviewPort } from '@/features/core/dockview';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { isGraphResourceDirty, markResourceDirty } from '@/features/core/resource';
import { logger } from '@/utils/appLogger';
import { warnCallFunctionIssuesBeforeSave } from '@/features/application/graphDiagnostics/warnCallFunctionIssues';
import {
  captureSettledGraphSaveCommandContext,
  isGraphSaveCommandRevisionCurrent,
  type GraphSaveCommandContext,
} from '@/features/application/projectCommandContext';
import { layoutTabFromDockviewPanel } from './dockviewTabProjection';
import { resolveTabDisplayName } from './resolveTabDisplayName';

interface DirtyEditorDocument {
  graphPath: string;
  title: string;
  type: 'event' | 'function' | 'worksheet';
}

function collectDirtyEditorDocuments(): DirtyEditorDocument[] {
  const seen = new Set<string>();
  const dirty: DirtyEditorDocument[] = [];
  for (const panel of editorDockviewPort.listPanels()) {
    const tab = layoutTabFromDockviewPanel(panel);
    if (!tab || (tab.type !== 'event' && tab.type !== 'function' && tab.type !== 'worksheet')) continue;
    if (seen.has(tab.id) || !isGraphResourceDirty(tab.id, tab.type)) continue;
    seen.add(tab.id);
    dirty.push({
      graphPath: tab.id,
      title: resolveTabDisplayName({ id: tab.id, kind: tab.type }, tab.id),
      type: tab.type,
    });
  }
  return dirty;
}

/** Persist every dirty document currently projected by Dockview. */
export async function saveAllDirtyGraphs(): Promise<boolean> {
  const dirty = collectDirtyEditorDocuments();
  if (dirty.length === 0) return true;

  for (const tab of dirty) {
    let context: GraphSaveCommandContext | undefined;
    try {
      if (tab.type === 'worksheet') {
        const saved = await useWorksheetStore.getState().saveDocument(tab.graphPath);
        if (!saved) return false;
        continue;
      }

      warnCallFunctionIssuesBeforeSave(tab.graphPath);
      context = await captureSettledGraphSaveCommandContext(tab.graphPath);
      await GraphService.saveProjectGraph(
        context.projectInstanceId,
        tab.graphPath,
        context.expectedRevision,
        context.operationId,
      );
      if (!isGraphSaveCommandRevisionCurrent(context, tab.graphPath)) return false;
      markResourceDirty({ id: tab.graphPath, kind: tab.type }, false);
    } catch (error) {
      if (context && !context.isCurrent()) return false;
      const message = error instanceof Error ? error.message : String(error);
      logger.app.error(
        `Failed to save graph '${tab.title}' (${tab.graphPath}): ${message}`,
        'saveAllDirtyGraphs',
      );
      logger.notify.error(`保存「${tab.title}」失败：${message}`, 'UI');
      return false;
    }
  }
  return true;
}
