import { memo } from "react";
import { useTranslation } from "react-i18next";
import { useConnection, type EdgeProps } from "@xyflow/react";
import { useShallow } from "zustand/react/shallow";
import { useGraphRead } from "@/features/core/graph/read";
import { graphElementState, useGraphResultPresentation } from "@/features/application/results";
import { useTheme } from "@/features/core/theme/useTheme";
import { getPinTypeColor } from "@/features/core/theme/pinTypeTheme";
import { resolvePinVisualSpec } from "@/shared/types/domain/pinVisual";
import { useGraphFlowContext, useGraphFlowInteraction } from "./GraphFlowContext";
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
  const { interactive, graphPath } = useGraphFlowContext();
  const { sourcePin, feedbackForPin } = useGraphFlowInteraction();
  const { t } = useTranslation();
  const replaced = useConnection((connection) => {
    const target = connection.toHandle?.id;
    const feedback = target ? feedbackForPin(target) : null;
    return feedback?.kind === "replace" && feedback.displacedConnectionIds.includes(id);
  });
  const { tokens } = useTheme();
  const pin = useGraphRead((snapshot) =>
    data ? snapshot.graphEntities[graphPath]?.pins[data.fromPinId] : undefined,
  );
  const input = useGraphRead((snapshot) =>
    data ? snapshot.graphEntities[graphPath]?.pins[data.toPinId] : undefined,
  );
  const blocked = useGraphRead(
    (snapshot) =>
      snapshot.graphEntities[graphPath]?.diagnostics.some(
        (diagnostic) =>
          diagnostic.blocking &&
          diagnostic.location.kind === "connection" &&
          diagnostic.location.connectionId === id,
      ) ?? false,
  );
  const [cache, state] = useGraphResultPresentation(
    graphPath,
    useShallow((presentation) => {
      const cache = pin && input ? (presentation.connections[pin.id]?.[input.id] ?? "new") : "new";
      return [
        cache,
        graphElementState(
          presentation,
          target,
          cache,
          blocked ||
            pin?.orphan ||
            input?.orphan ||
            presentation.failure?.source?.nodeId === source,
        ),
      ] as const;
    }),
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
