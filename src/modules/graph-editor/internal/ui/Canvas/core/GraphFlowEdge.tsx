import { memo, useSyncExternalStore } from "react";
import { useConnection, type EdgeProps } from "@xyflow/react";
import {
  connectionKey,
  getExecutionVisual,
  subscribeExecutionVisual,
} from "@/features/core/execution";
import { useExecutionRead } from "@/features/core/execution/read";
import { useTheme } from "@/features/core/theme/useTheme";
import { getPinTypeColor } from "@/features/core/theme/pinTypeTheme";
import { resolvePinVisualSpec } from "@/shared/types/domain/pinVisual";
import { useGraphFlowContext } from "./GraphFlowContext";
import type { GraphFlowEdge as FlowEdge } from "./graphFlowModel";
import { Edge } from "./Edge";

export const GraphFlowEdge = memo(function GraphFlowEdge({
  id,
  source,
  sourceX,
  sourceY,
  targetX,
  targetY,
  selected,
  data,
}: EdgeProps<FlowEdge>) {
  const { graphPath, interactive, model, sourcePin, feedbackForPin } = useGraphFlowContext();
  const targetHandle = useConnection((connection) => connection.toHandle?.id ?? null);
  const feedback = targetHandle ? feedbackForPin(targetHandle) : null;
  const replaced = feedback?.kind === "replace" && feedback.displacedConnectionIds.includes(id);
  const visual = useSyncExternalStore(
    subscribeExecutionVisual,
    getExecutionVisual,
    getExecutionVisual,
  );
  const graphState = useExecutionRead((snapshot) => snapshot.graphs[graphPath]);
  const isReplay = useExecutionRead(
    (snapshot) => snapshot.isPlaying && snapshot.playbackGraphPath === graphPath,
  );
  const { tokens } = useTheme();
  const useVisual = (visual.active && visual.graphPath === graphPath) || isReplay;
  const status = useVisual ? visual.status : (graphState?.status ?? "idle");
  const key = connectionKey(data?.fromPinId ?? "", data?.toPinId ?? "");
  const isError = useVisual
    ? visual.errorNodeIds.has(source)
    : graphState?.nodeStates?.get(source)?.status === "error";
  const hasFlow =
    (useVisual ? visual.flowingConnections : graphState?.flowingConnections)?.has(key) ?? false;
  const hasPull =
    (useVisual ? visual.completedConnections : graphState?.completedConnections)?.has(key) ?? false;
  const pin = data ? model.pins[data.fromPinId] : undefined;
  const color = pin
    ? getPinTypeColor(resolvePinVisualSpec(pin).colorKey, tokens)
    : tokens.mutedForeground;
  return (
    <g data-replacement-preview={replaced || undefined}>
      <Edge
        edgeId={id}
        x1={sourceX}
        y1={sourceY}
        x2={targetX}
        y2={targetY}
        color={color}
        isPullActive={hasPull && !hasFlow && !isError}
        startIsInput={pin?.direction === "input"}
        isFlowActive={hasFlow}
        isError={isError}
        isRunning={status === "running"}
        dimmed={sourcePin !== null}
        replacementPreview={replaced}
        selected={selected}
        interactive={interactive}
      />
    </g>
  );
});
