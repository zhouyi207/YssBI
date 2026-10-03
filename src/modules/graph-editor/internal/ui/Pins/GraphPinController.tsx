import {
  memo,
  useContext,
  useMemo,
  useState,
  type ComponentProps,
  type CSSProperties,
  type ReactNode,
} from "react";
import { useShallow } from "zustand/react/shallow";
import { graphElementState, useGraphResultPresentation } from "@/features/application/results";
import { GraphFlowContext } from "../Canvas/core/GraphFlowContext";
import { useTranslation } from "react-i18next";
import type { GraphContextMenuActions } from "@/features/application/editor";
import { openPinInspectableView } from "@/features/application/execution/openInspectableResult";
import { hasPinViewTarget } from "@/features/core/execution/pinViewTarget";
import { useGraphRead } from "@/features/core/graph/read";
import { getPinTypeColor } from "@/features/core/theme/pinTypeTheme";
import { useTheme } from "@/features/core/theme/useTheme";
import type { PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import { resolveNodePinDisplayLabel } from "@/features/domain/editorProjection/displayLabels";
import {
  formatGraphDiagnostic,
  isUnboundInputDiagnostic,
} from "@/features/domain/graphDiagnostics/nodeDiagnostics";
import { scalarPinInputKey } from "@/shared/types/domain/pinSemantics";
import { resolvePinRenderStyle, resolvePinVisualSpec } from "@/shared/types/domain/pinVisual";
import { PinContextMenu } from "../ContextMenu";
import { GraphPinView } from "./GraphPinView";
import { PinInput } from "./PinInput";

export interface GraphPinControllerProps {
  pin: PinData;
  graphPath?: string;
  contextMenuActions?: GraphContextMenuActions | null;
  renderPinHandle?: (pin: PinData) => ReactNode;
}

// Only a mounted menu subscribes to result-target availability.
function GraphPinContextMenu({
  graphPath,
  pin,
  ...props
}: Pick<GraphPinControllerProps, "graphPath" | "pin"> &
  Omit<ComponentProps<typeof PinContextMenu>, "showView" | "onView">) {
  const target = graphPath ? { graphPath, address: pin.address, direction: pin.direction } : null;
  const showView = useGraphRead(
    (snapshot) =>
      target !== null && hasPinViewTarget(target, snapshot.graphEntities[target.graphPath]),
  );
  return (
    <PinContextMenu
      {...props}
      showView={showView}
      onView={() => {
        if (target) void openPinInspectableView(target);
      }}
    />
  );
}

export const GraphPinController = memo(function GraphPinController(props: GraphPinControllerProps) {
  const { pin, graphPath, contextMenuActions, renderPinHandle } = props;
  // A stable factory lets unchanged sibling Pins skip rendering; local updates reuse their handle.
  const handleSlot = useMemo(() => renderPinHandle?.(pin), [pin, renderPinHandle]);
  const { id, nodeId, name, direction, orphan, input, typeState } = pin;
  const dataType = typeState.status === "exact" ? (typeState.dataType ?? undefined) : undefined;
  const defaultValue = input?.protocolDefault;
  const userValue = input?.literalOverride;
  const { t, i18n } = useTranslation();
  const inCanvas = useContext(GraphFlowContext) !== null;
  const { tokens } = useTheme();
  const isConnected = pin.connections.current > 0;
  const connectedSource = useGraphRead((snapshot) => {
    if (!graphPath || direction !== "input") return null;
    const bucket = snapshot.graphEntities[graphPath];
    if (!bucket) return null;
    return (
      (bucket.pinConnections[id] ?? [])
        .flatMap((connectionId) => {
          const connection = bucket.connections[connectionId];
          const label = connection && resolveNodePinDisplayLabel(bucket, connection.output);
          return label ? [label] : [];
        })
        .join(" · ") || null
    );
  });
  const pinSemantics = useMemo(() => ({ typeState }), [typeState]);
  const visualSpec = useMemo(() => resolvePinVisualSpec(pinSemantics), [pinSemantics]);
  const baseColor = getPinTypeColor(visualSpec.colorKey, tokens);
  const renderStyle = useMemo(
    () => resolvePinRenderStyle(isConnected, baseColor, tokens.mutedForeground),
    [baseColor, isConnected, tokens.mutedForeground],
  );

  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);

  const pinDiagnostic = useGraphRead((snapshot) =>
    graphPath ? snapshot.graphEntities[graphPath]?.primaryPortDiagnostics[id] : undefined,
  );
  const diagnosticMessage = useMemo(
    () =>
      pinDiagnostic ? formatGraphDiagnostic(pinDiagnostic, i18n?.resolvedLanguage) : undefined,
    [pinDiagnostic, i18n?.resolvedLanguage],
  );
  const [cacheState, executionState] = useGraphResultPresentation(
    graphPath,
    useShallow((presentation) => {
      const nodeCache = presentation.nodes[nodeId];
      const cache =
        direction === "output"
          ? (presentation.outputs[id] ?? "new")
          : (presentation.inputs[id] ??
            (nodeCache?.valid ? "valid" : nodeCache?.stale ? "stale" : "new"));
      return [
        cache,
        inCanvas
          ? graphElementState(presentation, nodeId, cache, pinDiagnostic?.blocking)
          : undefined,
      ] as const;
    }),
  );
  const editableInput = direction === "input" && scalarPinInputKey(dataType) !== null;
  const canReset = editableInput && userValue != null;
  const shouldPulse =
    !isConnected && direction === "input" && isUnboundInputDiagnostic(pinDiagnostic);
  const dragStyle: CSSProperties | undefined = orphan
    ? { opacity: 0.25, transition: "opacity 150ms, filter 150ms" }
    : undefined;
  const pinTooltip = diagnosticMessage
    ? `${name} (${visualSpec.label}) — ${diagnosticMessage}`
    : `${name} (${visualSpec.label})`;
  const tooltip = [
    pinTooltip,
    connectedSource,
    executionState && t(`canvas.graphState.${executionState}`),
    executionState === "error" && cacheState !== "new" && t(`canvas.graphState.${cacheState}`),
  ]
    .filter(Boolean)
    .join(" · ");

  const inputValue = userValue ?? defaultValue;
  // Runtime decoration does not change the editor's value, identity or local editing state.
  const inputSlot = useMemo(
    () =>
      editableInput && !isConnected && graphPath && nodeId ? (
        <PinInput
          pinId={id}
          nodeId={nodeId}
          graphPath={graphPath}
          dataType={dataType}
          value={inputValue}
        />
      ) : null,
    [editableInput, isConnected, graphPath, nodeId, id, dataType, inputValue],
  );
  const contextMenuSlot = contextMenu ? (
    <GraphPinContextMenu
      graphPath={graphPath}
      pin={pin}
      position={contextMenu}
      hasLinks={isConnected}
      canReset={canReset}
      onBreakLinks={
        contextMenuActions ? () => void contextMenuActions.disconnectPin(id) : undefined
      }
      onResetValue={
        contextMenuActions ? () => void contextMenuActions.resetPinValue(nodeId, id) : undefined
      }
      onClose={() => setContextMenu(null)}
    />
  ) : null;

  return (
    <GraphPinView
      id={id}
      name={name}
      direction={direction}
      isConnected={isConnected}
      contextMenuOpen={contextMenu != null}
      diagnosticMessage={diagnosticMessage}
      dragStyle={dragStyle}
      handleSlot={handleSlot}
      visualSpec={visualSpec}
      renderStyle={renderStyle}
      baseColor={baseColor}
      shouldPulse={shouldPulse}
      executionState={executionState}
      cacheState={cacheState}
      tooltip={tooltip}
      inputSlot={inputSlot}
      contextMenuSlot={contextMenuSlot}
      onContextMenu={(event) => {
        event.preventDefault();
        event.stopPropagation();
        setContextMenu({ x: event.clientX, y: event.clientY });
      }}
      onClick={(event) => {
        event.stopPropagation();
      }}
    />
  );
});
