import { memo } from "react";
import { useTranslation } from "react-i18next";
import { useConnection, type EdgeProps } from "@xyflow/react";
import { graphElementState } from "@/features/application/results";
import { useTheme } from "@/features/core/theme/useTheme";
import { getPinTypeColor } from "@/features/core/theme/pinTypeTheme";
import { resolvePinVisualSpec } from "@/shared/types/domain/pinVisual";
import { useGraphFlowContext } from "./GraphFlowContext";
import type { GraphFlowEdge as FlowEdge } from "./graphFlowModel";
import { Edge } from "./Edge";

export const GraphFlowEdge = memo(function GraphFlowEdge({
  id,
  source,
  target,
  sourceX,
  sourceY,
  targetX,
  targetY,
  selected,
  data,
}: EdgeProps<FlowEdge>) {
  const { interactive, model, sourcePin, feedbackForPin, presentation, blockedConnections } =
    useGraphFlowContext();
  const { t } = useTranslation();
  const targetHandle = useConnection((connection) => connection.toHandle?.id ?? null);
  const feedback = targetHandle ? feedbackForPin(targetHandle) : null;
  const replaced = feedback?.kind === "replace" && feedback.displacedConnectionIds.includes(id);
  const { tokens } = useTheme();
  const pin = data ? model.pins[data.fromPinId] : undefined;
  const input = data ? model.pins[data.toPinId] : undefined;
  const cache = pin && input ? (presentation.connections[pin.id]?.[input.id] ?? "new") : "new";
  const state = graphElementState(
    presentation,
    target,
    cache,
    blockedConnections.has(id) ||
      pin?.orphan ||
      input?.orphan ||
      presentation.failure?.source?.nodeId === source,
  );
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
        state={state}
        cacheState={cache}
        title={t(`canvas.graphState.${state}`)}
        startIsInput={pin?.direction === "input"}
        dimmed={sourcePin !== null}
        replacementPreview={replaced}
        selected={selected}
        interactive={interactive}
      />
    </g>
  );
});
