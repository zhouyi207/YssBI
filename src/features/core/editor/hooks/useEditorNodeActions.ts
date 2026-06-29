/**
 * 编辑器节点操作：setNodes、setSelectedNodeIds
 */
import { useCallback, RefObject } from 'react';
import { buildRuntimeNodesFromStore, useGraphDataStore } from '@/features/core/dataStore';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import type { LayoutState } from '@/features/core/layout/layoutStore';
import { syncDetailFromNodeSelection } from '@/features/core/editor';
import { Node } from '@/shared/types/ui';

function areStringArraysEqual(a: string[], b: string[]) {
  if (a.length !== b.length) return false;
  const set = new Set(a);
  return b.every((value) => set.has(value));
}

export function useEditorNodeActions(
  activeTabIdRef: RefObject<string | null>,
  activeGroupId: string
) {
  const setNodes = useCallback((updater: Node[] | ((prev: Node[]) => Node[])) => {
    const tId = activeTabIdRef.current;
    if (!tId) return;

    const currentNodes = buildRuntimeNodesFromStore(tId);
    const nextNodes = typeof updater === 'function' ? updater(currentNodes as unknown as Node[]) : updater;

    useGraphDataStore.getState().replaceGraphNodes(tId, nextNodes as import('@/shared/types/store/graph').RuntimeNodeInput[]);
  }, [activeTabIdRef]);

  const setSelectedNodeIds = useCallback(
    (updater: string[] | ((prev: string[]) => string[]), targetGroupId?: string) => {
      const gid = targetGroupId || activeGroupId;
      if (gid) {
        const state = useLayoutStore.getState() as LayoutState;
        const node = state.nodes[gid];
        if (node) {
          const current = node.data?.params?.selectedNodeIds || [];
          const next = typeof updater === 'function' ? updater(current) : updater;
          if (areStringArraysEqual(current, next)) return;

          useLayoutStore.getState().updateNode(gid, {
            data: {
              ...node.data,
              params: { ...node.data?.params, selectedNodeIds: next },
            },
          });
          syncDetailFromNodeSelection(gid, next);
        }
      }
    },
    [activeGroupId]
  );

  return { setNodes, setSelectedNodeIds };
}
