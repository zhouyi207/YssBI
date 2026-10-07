import { memo, useMemo, type RefCallback } from "react";
import { useTranslation } from "react-i18next";
import { Position, type EdgeProps } from "@xyflow/react";
import { useShallow } from "zustand/react/shallow";
import { useGraphRead } from "@/features/core/graph/read";
import { graphElementState, useGraphResultPresentation } from "@/features/application/results";
import { useTheme } from "@/features/core/theme/useTheme";
import { getPinTypeColor } from "@/features/core/theme/pinTypeTheme";
import { computeEdgePath } from "@/features/core/canvas";
import { useGraphFlowContext, useGraphFlowInteraction } from "./GraphFlowContext";
import {
  createGraphFlowEdgeAppearanceSelector,
  type GraphFlowEdge as FlowEdge,
} from "./graphFlowModel";
import { Edge } from "./Edge";
import { useEdgePath } from "./useEdgePath";

export const GraphFlowEdge = memo(function GraphFlowEdge({
  id,
  source,
  target,
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  selected,
  data,
}: EdgeProps<FlowEdge>) {
  const pathRef = useEdgePath(
    computeEdgePath(sourceX, sourceY, targetX, targetY, sourcePosition === Position.Left),
  );
  return useMemo(
    () => (
      <GraphFlowEdgeAppearance
        id={id}
        source={source}
        target={target}
        selected={selected}
        data={data}
        pathRef={pathRef}
      />
    ),
    [id, source, target, selected, data, pathRef],
  );
});

// Coordinate updates only write path geometry above. Business subscriptions and
// the visible, highlighted and hit shapes stay mounted without rerendering on each move.
const GraphFlowEdgeAppearance = memo(function GraphFlowEdgeAppearance({
  id,
  source,
  target,
  selected,
  data,
  pathRef,
}: Pick<EdgeProps<FlowEdge>, "id" | "source" | "target" | "selected" | "data"> & {
  pathRef: RefCallback<SVGPathElement>;
}) {
  const { interactive, graphPath } = useGraphFlowContext();
  const { t } = useTranslation();
  const replaced = useGraphFlowInteraction((state) => state.replacedConnectionIds.has(id));
  const { tokens } = useTheme();
  const fromPinId = data?.fromPinId;
  const toPinId = data?.toPinId;
  const selectAppearance = useMemo(
    () => createGraphFlowEdgeAppearanceSelector(graphPath, id, fromPinId, toPinId),
    [graphPath, id, fromPinId, toPinId],
  );
  const { colorKey, hasEndpoints, blocked } = useGraphRead(selectAppearance);
  const [cache, state] = useGraphResultPresentation(
    graphPath,
    useShallow((presentation) => {
      const cache =
        hasEndpoints && data
          ? (presentation.connections[data.fromPinId]?.[data.toPinId] ?? "new")
          : "new";
      return [
        cache,
        graphElementState(
          presentation,
          target,
          cache,
          blocked || presentation.failure?.source?.nodeId === source,
        ),
      ] as const;
    }),
  );
  const color = colorKey === undefined ? tokens.mutedForeground : getPinTypeColor(colorKey, tokens);
  return (
    <g data-replacement-preview={replaced || undefined}>
      <Edge
        edgeId={id}
        pathRef={pathRef}
        color={color}
        state={state}
        cacheState={cache}
        title={t(`canvas.graphState.${state}`)}
        replacementPreview={replaced}
        selected={selected}
        interactive={interactive}
      />
    </g>
  );
});
