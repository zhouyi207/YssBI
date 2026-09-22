/**
 * useNodeView - 单节点订阅 Hook
 *
 * 为画布上的「每个节点」提供一个**仅订阅自身切片**的视图：
 *   - graph-scoped node bucket: 位置 / 标题 / 类型 / 参数
 *   - graph-scoped pins        输入/输出 Pin
 */
import { useMemo } from "react";
import { useShallow } from "zustand/react/shallow";
import { useGraphProjectionStore } from "./graphProjectionStore";
import { toUiNode, type UINode } from "./nodeView";

export function useNodeView(nodeId: string, graphPath?: string): UINode | null {
  const nodeData = useGraphProjectionStore((s) =>
    graphPath ? s.getGraphNode(graphPath, nodeId) : undefined,
  );

  const pinObjs = useGraphProjectionStore(
    useShallow((s) =>
      graphPath
        ? s.getGraphNodePins(graphPath, nodeId).map((pid) => s.getGraphPin(graphPath, pid))
        : [],
    ),
  );

  return useMemo(() => {
    if (!graphPath || !nodeData) return null;

    return toUiNode(
      nodeData,
      pinObjs.filter((pin) => pin !== undefined),
    );
  }, [graphPath, nodeData, pinObjs]);
}
