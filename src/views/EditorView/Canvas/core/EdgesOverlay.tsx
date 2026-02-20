import React, { useMemo } from "react";
import { Edge } from "./Edge";
import { useExecutionStore } from "@/features/core/execution";
import { useTheme } from "@/features/core/theme/useTheme";
import { getPinTypeColor } from "@/features/core/theme/pinTypeTheme";
import type { Pin } from "@/shared/types/domain";

interface EdgesOverlayProps {
  nodes: Array<{
    id: string;
    outputs: Pin[];
  }>;
  getPinWorldPos: (pinId: string) => { x: number; y: number } | null;
}

interface EdgeData {
  id: string;
  fromPinId: string;
  toPinId: string;
  pinType: string;
  pinColor?: string;
}

/**
 * SVG 边渲染层 — 与节点共享 TransformContainer 的坐标空间
 *
 * 优势：
 * - 和 Node/Pin 使用同一个 CSS transform，天然同步
 * - React 驱动的声明式渲染，每条边独立 memo
 * - 无需手动应用 viewport 变换
 */
export const EdgesOverlay = React.memo<EdgesOverlayProps>(({ nodes, getPinWorldPos }) => {
  const { theme } = useTheme();
  const activeConnections = useExecutionStore((s) => s.activeConnections);
  const completedConnections = useExecutionStore((s) => s.completedConnections);

  const edges = useMemo<EdgeData[]>(() => {
    const result: EdgeData[] = [];
    for (const node of nodes) {
      if (!node?.outputs) continue;
      for (const pin of node.outputs) {
        if (!pin.links) continue;
        for (const targetId of pin.links) {
          result.push({
            id: `${pin.id}->${targetId}`,
            fromPinId: pin.id,
            toPinId: targetId,
            pinType: pin.type ?? "any",
            pinColor: pin.ui?.color,
          });
        }
      }
    }
    return result;
  }, [nodes]);

  return (
    <svg
      className="absolute pointer-events-none"
      style={{ overflow: "visible", left: 0, top: 0, width: 1, height: 1, zIndex: 0 }}
    >
      <style>{`@keyframes dash { to { stroke-dashoffset: -20; } }`}</style>
      {edges.map((edge) => {
        const start = getPinWorldPos(edge.fromPinId);
        const end = getPinWorldPos(edge.toPinId);
        if (!start || !end) return null;

        const isActive = activeConnections.has(edge.id);
        const isCompleted = completedConnections.has(edge.id);
        const color = edge.pinColor ?? getPinTypeColor(edge.pinType, theme);

        return (
          <Edge
            key={edge.id}
            x1={start.x}
            y1={start.y}
            x2={end.x}
            y2={end.y}
            color={isActive ? "#facc15" : isCompleted ? "#10b981" : color}
            thickness={2}
            isActive={isActive}
          />
        );
      })}
    </svg>
  );
});
