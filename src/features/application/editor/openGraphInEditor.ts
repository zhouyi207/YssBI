import { Graph } from '@/shared/types/domain';
import { getGraphById, useProjectIOStore } from '@/features/core/dataStore';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useEditorStore } from '@/features/core/editor';
import { syncVariablesGraphScopeFromActiveTab } from '@/features/core/editor/detail/variablesGraphScope';
import { ensureGraphViewport } from '@/features/core/viewport';
import { logger } from '@/utils/appLogger';

export async function openGraphInEditor(
  id: string,
  name: string,
  type: 'event' | 'function',
  targetGroupId?: string,
  initialData?: Graph,
): Promise<void> {
  logger.graph.trace(`openGraphInEditor called: id=${id}, name=${name}, type=${type}`, 'TabManagement');

  if (!initialData) {
    const loaded = await useProjectIOStore.getState().loadGraph(id);
    if (!loaded) return;
  }

  const layoutStore = useLayoutStore.getState();
  const groupId = targetGroupId ?? layoutStore.activeEditorGroupId ?? layoutStore.activeGroupId ?? 'default_editor';

  layoutStore.addTab(groupId, {
    id,
    title: name,
    component: 'GraphEditor',
    type,
  });

  layoutStore.setActiveGroup(groupId);

  const tabSource = initialData || getGraphById(id);
  ensureGraphViewport(id, tabSource?.canvas);

  useEditorStore.getState().setDetailFocus({ kind: type, id });
  syncVariablesGraphScopeFromActiveTab();
}
