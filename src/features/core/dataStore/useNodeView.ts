/**
 * useNodeView - 单节点订阅 Hook
 *
 * 为画布上的「每个节点」提供一个**仅订阅自身切片**的视图：
 *   - graph-scoped node bucket: 位置 / 标题 / 类型 / 参数
 *   - graph-scoped pins        输入/输出 Pin
 *   - graph-scoped connections 连接状态（派生 connected / connectionIds）
 */
import { useMemo } from "react";
import { useShallow } from "zustand/react/shallow";
import { useGraphDataStore } from "./graphDataStore";
import { toUiNode, type UINode } from "./nodeView";

export function useNodeView(nodeId: string, graphPath?: string): UINode | null {
  const nodeData = useGraphDataStore((s) =>
    graphPath ? s.getGraphNode(graphPath, nodeId) : undefined,
  );

  const pinObjs = useGraphDataStore(
    useShallow((s) =>
      graphPath
        ? s.getGraphNodePins(graphPath, nodeId).map((pid) => s.getGraphPin(graphPath, pid))
        : [],
    ),
  );

  const pinConns = useGraphDataStore(
    useShallow((s) =>
      graphPath
        ? s
            .getGraphNodePins(graphPath, nodeId)
            .map((pid) => s.getGraphPinConnections(graphPath, pid))
        : [],
    ),
  );

  return useMemo(() => {
    if (!graphPath || !nodeData) return null;

    const pins = pinObjs.flatMap((pin, index) =>
      pin ? [{ pin, connectionIds: pinConns[index] ?? [] }] : [],
    );

    return toUiNode(nodeData, { pins });
  }, [graphPath, nodeData, pinObjs, pinConns]);
}
