import { Graph } from '@/shared/types/domain';
import { getGraphById, useProjectIOStore } from '@/features/core/dataStore';
import { syncVariablesGraphScopeFromActiveTab } from '@/features/core/editor/detail/variablesGraphScope';
import { ensureGraphViewport } from '@/features/core/viewport';
import { logger } from '@/utils/appLogger';
import { buildGraphLayoutTab } from '@/features/core/layout/layoutTabModel';
import { openEditorTab } from './openEditorTab';

export async function openGraphInEditor(
  id: string,
  name: string,
  type: 'event' | 'function',
  targetGroupId?: string,
  initialData?: Graph,
): Promise<void> {
  logger.graph.trace(`openGraphInEditor called: id=${id}, name=${name}, type=${type}`, 'TabManagement');

  // Always hydrate the Rust backend before editing; frontend graphEntities alone is not authoritative.
  if (!initialData) {
    const loaded = await useProjectIOStore.getState().loadGraph(id);
    if (!loaded) return;
  }

  openEditorTab(buildGraphLayoutTab(id, name, type), {
      targetGroupId,
      focusDetail: { kind: type, id },
    });

  const tabSource = initialData || getGraphById(id);
  ensureGraphViewport(id, tabSource?.canvas);
  syncVariablesGraphScopeFromActiveTab();
}
