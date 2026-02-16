/**
 * 编辑器节点操作：setNodes、setSelectedNodeIds
 */
import { useCallback, RefObject } from 'react';
import { getGraphById, useGraphDataStore } from '@/features/core/dataStore';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import type { LayoutState } from '@/features/core/layout/layoutStore';
import { Node } from '@/shared/types/ui';
import { deserializeGraph } from '@/shared/utils/editor';

export function useEditorNodeActions(
  activeTabIdRef: RefObject<string | null>,
  activeGroupId: string
) {
  const setNodes = useCallback((updater: Node[] | ((prev: Node[]) => Node[])) => {
    const tId = activeTabIdRef.current;
    if (!tId) return;
    const graphData = getGraphById(tId);
    if (!graphData) return;

    const { nodes: currentNodes } = deserializeGraph(graphData);
    const nextNodes = typeof updater === 'function' ? updater(currentNodes) : updater;

    useGraphDataStore.getState().replaceGraphNodes(tId, nextNodes as any);
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
          useLayoutStore.getState().updateNode(gid, {
            data: {
              ...node.data,
              params: { ...node.data?.params, selectedNodeIds: next },
            },
          });
        }
      }
    },
    [activeGroupId]
  );

  return { setNodes, setSelectedNodeIds };
}
