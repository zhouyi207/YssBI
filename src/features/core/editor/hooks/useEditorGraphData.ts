/**
 * 获取指定 Tab 的图数据（nodes, variables）
 * 依赖 dataStore
 */

import { useMemo } from 'react';
import { useGraphData } from '@/features/core/dataStore';
import { deserializeGraph } from '@/features/core/dataStore';

export function useEditorGraphData(activeTabId: string | null) {
  const graphData = useGraphData(activeTabId);

  return useMemo(() => {
    if (!graphData) return { nodes: [], variables: {} };
    return deserializeGraph(graphData);
  }, [graphData]);
}
