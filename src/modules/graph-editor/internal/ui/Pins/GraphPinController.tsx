import {
  useCallback,
  useContext,
  useMemo,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";
import { useShallow } from "zustand/react/shallow";
import { graphElementState, useGraphResultPresentation } from "@/features/application/results";
import { GraphFlowContext } from "../Canvas/core/GraphFlowContext";
import { useTranslation } from "react-i18next";
import type { GraphContextMenuActions } from "@/features/application/editor";
import { openPinInspectableView } from "@/features/application/execution/openInspectableResult";
import {
  inspectableRefsFromPinView,
  type ResolvePinViewTargetParams,
} from "@/features/core/execution/pinViewTarget";
import { useGraphRead } from "@/features/core/graph/read";
import { getPinTypeColor } from "@/features/core/theme/pinTypeTheme";
import { useTheme } from "@/features/core/theme/useTheme";
import type { PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import {
  findPrimaryPortDiagnostic,
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
  handleSlot?: ReactNode;
}

export function GraphPinController(props: GraphPinControllerProps) {
  const { pin, graphPath, contextMenuActions, handleSlot } = props;
  const { id, nodeId, name, direction, address, orphan, input, typeState } = pin;
  const dataType = typeState.status === "exact" ? (typeState.dataType ?? undefined) : undefined;
  const defaultValue = input?.protocolDefault;
  const userValue = input?.literalOverride;
  const { t, i18n } = useTranslation();
  const inCanvas = useContext(GraphFlowContext) !== null;
  const { tokens } = useTheme();
  const isConnected = pin.connections.current > 0;
  const pinSemantics = useMemo(() => ({ typeState }), [typeState]);
  const visualSpec = useMemo(() => resolvePinVisualSpec(pinSemantics), [pinSemantics]);
  const baseColor = getPinTypeColor(visualSpec.colorKey, tokens);
  const renderStyle = useMemo(
    () => resolvePinRenderStyle(isConnected, baseColor, tokens.mutedForeground),
    [baseColor, isConnected, tokens.mutedForeground],
  );

  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);

  const graphConnections = useGraphRead(
    useShallow((snapshot) => {
      const bucket = graphPath ? snapshot.graphEntities[graphPath] : undefined;
      if (!bucket || direction === "output") return [];
      return (bucket.pinConnections[id] ?? []).map(
        (connectionId) => bucket.connections[connectionId],
      );
    }),
  );
  const pinDiagnostic = useGraphRead((snapshot) => {
    const diagnostics = graphPath
      ? snapshot.graphEntities[graphPath]?.nodes[nodeId]?.diagnostics
      : undefined;
    return diagnostics ? findPrimaryPortDiagnostic(diagnostics, address) : undefined;
  });
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
  const viewParams = useMemo<ResolvePinViewTargetParams | null>(
    () =>
      graphPath
        ? {
            graphPath,
            address,
            direction,
            connections: graphConnections,
          }
        : null,
    [address, graphConnections, direction, graphPath],
  );
  const showViewMenu = viewParams !== null && inspectableRefsFromPinView(viewParams).length > 0;
  const handleView = useCallback(() => {
    if (viewParams) void openPinInspectableView(viewParams);
  }, [viewParams]);

  const editableInput = direction === "input" && scalarPinInputKey(dataType) !== null;
  const canReset = editableInput && userValue != null;
  const shouldPulse =
    !isConnected && direction === "input" && isUnboundInputDiagnostic(pinDiagnostic);
  const showInput = Boolean(editableInput && !isConnected && graphPath && nodeId);
  const dragStyle: CSSProperties | undefined = orphan
    ? { opacity: 0.25, transition: "opacity 150ms, filter 150ms" }
    : undefined;
  const pinTooltip = pinDiagnostic
    ? `${name} (${visualSpec.label}) — ${formatGraphDiagnostic(pinDiagnostic, i18n?.resolvedLanguage)}`
    : `${name} (${visualSpec.label})`;
  const tooltip = [
    pinTooltip,
    executionState && t(`canvas.graphState.${executionState}`),
    executionState === "error" && cacheState !== "new" && t(`canvas.graphState.${cacheState}`),
  ]
    .filter(Boolean)
    .join(" · ");

  const inputSlot = showInput ? (
    <PinInput
      pinId={id}
      nodeId={nodeId}
      graphPath={graphPath!}
      dataType={dataType}
      value={userValue ?? defaultValue}
    />
  ) : null;
  const contextMenuSlot = contextMenu ? (
    <PinContextMenu
      position={contextMenu}
      hasLinks={isConnected}
      canReset={canReset}
      onBreakLinks={
        contextMenuActions ? () => void contextMenuActions.disconnectPin(id) : undefined
      }
      onResetValue={
        contextMenuActions ? () => void contextMenuActions.resetPinValue(nodeId, id) : undefined
      }
      showView={showViewMenu}
      onView={handleView}
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
      diagnosticMessage={
        pinDiagnostic ? formatGraphDiagnostic(pinDiagnostic, i18n?.resolvedLanguage) : undefined
      }
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
}
