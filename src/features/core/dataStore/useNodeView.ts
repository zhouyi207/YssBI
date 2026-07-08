/**
 * useNodeView - 单节点订阅 Hook
 *
 * 为画布上的「每个节点」提供一个**仅订阅自身切片**的视图：
 *   - graph-scoped node bucket: 位置 / 标题 / 类型 / 参数
 *   - graph-scoped pins        输入/输出 Pin
 *   - graph-scoped connections 连接状态（派生 connected / connectionIds）
 */
import { useMemo } from 'react';
import { useShallow } from 'zustand/react/shallow';
import type { UINode } from '@/shared/types/ui';
import { useGraphDataStore } from './graphDataStore';
import { resourceKey, useResourceStore } from '@/features/core/resource';
import { toUiNode } from './nodeView';

import { CALL_FUNCTION_NODE_TYPE } from '@/features/domain/nodeDefinition';

export function useNodeView(nodeId: string, graphId?: string): UINode | null {
  const nodeData = useGraphDataStore((s) => (graphId ? s.getGraphNode(graphId, nodeId) : undefined));

  // Call Function 节点在画布上显示目标函数名（随函数重命名实时更新），而非静态 "Call Function"。
  // 名称以 ResourceStore 为准（重命名的单一事实来源）。
  const callFunctionName = useResourceStore((s) => {
    if (nodeData?.nodeType !== CALL_FUNCTION_NODE_TYPE || !nodeData.subGraphId) return undefined;
    const meta = s.resources[resourceKey({ id: nodeData.subGraphId, kind: 'function' })];
    return meta?.exists ? meta.name : undefined;
  });

  const pinObjs = useGraphDataStore(
    useShallow((s) =>
      graphId ? s.getGraphNodePins(graphId, nodeId).map((pid) => s.getGraphPin(graphId, pid)) : [],
    ),
  );

  const pinConns = useGraphDataStore(
    useShallow((s) =>
      graphId ? s.getGraphNodePins(graphId, nodeId).map((pid) => s.getGraphPinConnections(graphId, pid)) : [],
    ),
  );

  return useMemo(() => {
    if (!graphId || !nodeData) return null;

    const pins = pinObjs.flatMap((pin, index) =>
      pin ? [{ pin, connectionIds: pinConns[index] ?? [] }] : [],
    );

    return toUiNode(nodeData, {
      title: callFunctionName,
      pins,
    });
  }, [graphId, nodeData, pinObjs, pinConns, callFunctionName]);
}
