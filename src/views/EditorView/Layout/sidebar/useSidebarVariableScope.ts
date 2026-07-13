import { useMemo } from 'react';
import { useEditorStore } from '@/features/core/editor/stores/useEditorStore';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { lookupGraphResourceKind, useResourceStore } from '@/features/core/resource';
import { inferGraphResourceKind } from '@/shared/types/domain/graphResourcePath';
import type { GraphResourceType } from '../sidebarContextMenu';

/** Resolve the graph scope used for Local variables — aligned with useVariableManagement. */
export function useSidebarVariableScope(): {
  scopePath: string | null;
  graphType: GraphResourceType | undefined;
} {
  const activeEditorNode = useLayoutStore((s) =>
    s.activeEditorGroupId ? s.nodes[s.activeEditorGroupId] : null,
  );
  const activeTabId = activeEditorNode?.data?.activeTabId ?? null;
  const variablesGraphScopePath = useEditorStore((s) => s.variablesGraphScopePath);
  const scopePath = variablesGraphScopePath ?? activeTabId;

  const graphTypeFromTab =
    scopePath && activeEditorNode?.data?.tabs
      ? activeEditorNode.data.tabs.find((tab) => tab.id === scopePath)?.type
      : undefined;

  const graphTypeFromResource = useResourceStore((s) =>
    scopePath ? lookupGraphResourceKind(s.resources, scopePath) : undefined,
  );

  const rawType = graphTypeFromTab || graphTypeFromResource || (scopePath ? inferGraphResourceKind(scopePath) : undefined);

  const graphType =
    rawType === 'event' || rawType === 'function' ? rawType : undefined;

  return useMemo(
    () => ({ scopePath, graphType }),
    [scopePath, graphType],
  );
}
