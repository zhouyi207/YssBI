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
import { useResourceStore } from '@/features/core/resource';
import { CALL_FUNCTION_NODE_TYPE } from '@/features/domain/nodeDefinition';
import { getFunctionResourceName } from '@/features/domain/graphDiagnostics';
import { toUiNode } from './nodeView';

export function useNodeView(nodeId: string, graphPath?: string): UINode | null {
  const nodeData = useGraphDataStore((s) => (graphPath ? s.getGraphNode(graphPath, nodeId) : undefined));

  // Call Function 节点在画布上显示目标函数名（随函数重命名实时更新），而非静态 "Call Function"。
  // 名称以 ResourceStore 为准（重命名的单一事实来源）。
  const callFunctionName = useResourceStore((s) => {
    if (nodeData?.nodeType !== CALL_FUNCTION_NODE_TYPE || !nodeData.subGraphPath) return undefined;
    return getFunctionResourceName(s.resources, nodeData.subGraphPath);
  });

  const callTitleOverride =
    nodeData?.nodeType === CALL_FUNCTION_NODE_TYPE && nodeData.subGraphPath
      ? (callFunctionName ?? '(missing function)')
      : callFunctionName;

  const pinObjs = useGraphDataStore(
    useShallow((s) =>
      graphPath ? s.getGraphNodePins(graphPath, nodeId).map((pid) => s.getGraphPin(graphPath, pid)) : [],
    ),
  );

  const pinConns = useGraphDataStore(
    useShallow((s) =>
      graphPath ? s.getGraphNodePins(graphPath, nodeId).map((pid) => s.getGraphPinConnections(graphPath, pid)) : [],
    ),
  );

  return useMemo(() => {
    if (!graphPath || !nodeData) return null;

    const pins = pinObjs.flatMap((pin, index) =>
      pin ? [{ pin, connectionIds: pinConns[index] ?? [] }] : [],
    );

    return toUiNode(nodeData, {
      title: callTitleOverride,
      pins,
    });
  }, [graphPath, nodeData, pinObjs, pinConns, callTitleOverride]);
}
