import { useMemo } from 'react';
import { useActiveEditorGroup } from '@/features/core/editor/hooks/useActiveEditorGroup';
import { useEditorStore } from '@/features/core/editor/stores/useEditorStore';
import { lookupGraphResourceKind, useResourceStore } from '@/features/core/resource';
import { inferGraphResourceKind } from '@/shared/types/domain/graphResourcePath';
import type { GraphResourceType } from '../sidebarContextMenu';

/** Resolve the graph scope used for Local variables — aligned with useVariableManagement. */
export function useSidebarVariableScope(): {
  scopePath: string | null;
  graphType: GraphResourceType | undefined;
} {
  const { activeTabId, tabs } = useActiveEditorGroup();
  const variablesGraphScopePath = useEditorStore((s) => s.variablesGraphScopePath);
  const scopePath = variablesGraphScopePath ?? activeTabId;

  const graphTypeFromTab =
    scopePath
      ? tabs.find((tab) => tab.id === scopePath)?.type
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
