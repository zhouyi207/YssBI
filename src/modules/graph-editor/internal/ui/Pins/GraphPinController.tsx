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
import {
  isPinPreviewActionAvailable,
  requestAndOpenPinPreview,
} from "@/features/application/editor/requestPinPreview";
import { openPinInspectableView } from "@/features/application/execution/openInspectableResult";
import {
  buildPinViewParams,
  evaluatePinViewState,
  pinViewDisabledTitle,
} from "@/features/core/execution/pinViewTarget";
import { useGraphRead } from "@/features/core/graph/read";
import { getPinTypeColor } from "@/features/core/theme/pinTypeTheme";
import { useTheme } from "@/features/core/theme/useTheme";
import type { PinData, PinView } from "@/features/domain/editorProjection/graphRuntimeTypes";
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

export type GraphPinDragState = "normal" | "highlighted" | "dimmed";

const EMPTY_CONNECTION_IDS: string[] = [];

export interface GraphPinControllerProps {
  pin: PinData & Partial<Pick<PinView, "connected" | "linkCount">>;
  graphPath?: string;
  contextMenuActions?: GraphContextMenuActions | null;
  handleSlot?: ReactNode;
  isActive?: boolean;
  pinDragState?: GraphPinDragState;
}

export function GraphPinController(props: GraphPinControllerProps) {
  const {
    pin,
    graphPath,
    contextMenuActions,
    handleSlot,
    isActive,
    pinDragState = "normal",
  } = props;
  const {
    id,
    nodeId,
    name,
    direction,
    connected = false,
    linkCount = 0,
    address,
    orphan,
    status,
    input,
    typeState,
  } = pin;
  const dataType = typeState.status === "exact" ? (typeState.dataType ?? undefined) : undefined;
  const defaultValue = input?.protocolDefault;
  const userValue = input?.literalOverride;
  const { t, i18n } = useTranslation();
  const inCanvas = useContext(GraphFlowContext) !== null;
  const { tokens } = useTheme();
  const isConnected = connected || linkCount > 0 || (isActive ?? false);
  const pinSemantics = useMemo(() => ({ typeState }), [typeState]);
  const visualSpec = useMemo(() => resolvePinVisualSpec(pinSemantics), [pinSemantics]);
  const baseColor = getPinTypeColor(visualSpec.colorKey, tokens);
  const renderStyle = useMemo(
    () => resolvePinRenderStyle(isConnected, baseColor, tokens.mutedForeground),
    [baseColor, isConnected, tokens.mutedForeground],
  );

  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);

  const connectionIds = useGraphRead((snapshot) =>
    graphPath
      ? (snapshot.graphEntities[graphPath]?.pinConnections[id] ?? EMPTY_CONNECTION_IDS)
      : EMPTY_CONNECTION_IDS,
  );
  const graphConnections = useGraphRead(
    useShallow((snapshot) => {
      const bucket = graphPath ? snapshot.graphEntities[graphPath] : undefined;
      return (bucket?.pinConnections[id] ?? EMPTY_CONNECTION_IDS).map(
        (connectionId) => bucket?.connections[connectionId],
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
  const connections = useMemo(
    () =>
      graphConnections.flatMap((connection) =>
        connection?.output && connection.input
          ? [
              {
                connectionId: connection.id,
                output: connection.output,
                input: connection.input,
                order: connection.order ?? null,
              },
            ]
          : [],
      ),
    [graphConnections],
  );
  const viewParams = useMemo(
    () =>
      graphPath
        ? buildPinViewParams({
            graphPath,
            address,
            direction,
            connections,
          })
        : null,
    [address, connections, direction, graphPath],
  );
  const viewState = viewParams ? evaluatePinViewState(viewParams) : null;
  const previewActionAvailable = isPinPreviewActionAvailable(graphPath, {
    direction,
    address,
    orphan,
    status,
  });
  const showViewMenu = (viewState?.showMenu ?? false) || previewActionAvailable;
  const viewEnabled = (viewState?.enabled ?? false) || previewActionAvailable;
  const viewDisabledReason = previewActionAvailable ? null : (viewState?.disabledReason ?? null);
  const handleView = useCallback(() => {
    if (!viewParams || !graphPath) return;
    if (viewState?.enabled) {
      void openPinInspectableView(viewParams);
      return;
    }
    if (previewActionAvailable) {
      void requestAndOpenPinPreview(graphPath, id);
    }
  }, [graphPath, id, previewActionAvailable, viewParams, viewState?.enabled]);

  const hasLinks = linkCount > 0 || connectionIds.length > 0;
  const editableInput = direction === "input" && scalarPinInputKey(dataType) !== null;
  const canReset = editableInput && userValue != null;
  const shouldPulse =
    !isConnected && direction === "input" && isUnboundInputDiagnostic(pinDiagnostic);
  const showInput = Boolean(editableInput && !isConnected && graphPath && nodeId);
  const effectivePinDragState = contextMenu ? "highlighted" : pinDragState;
  const dragStyle: CSSProperties | undefined =
    effectivePinDragState === "dimmed"
      ? { opacity: 0.25, transition: "opacity 150ms, filter 150ms" }
      : effectivePinDragState === "highlighted"
        ? { filter: "brightness(1.25) saturate(1.4)", transition: "opacity 150ms, filter 150ms" }
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
      hasLinks={hasLinks}
      canReset={canReset}
      onBreakLinks={
        contextMenuActions ? () => void contextMenuActions.disconnectPin(id) : undefined
      }
      onResetValue={
        contextMenuActions ? () => void contextMenuActions.resetPinValue(nodeId, id) : undefined
      }
      showView={showViewMenu}
      viewEnabled={viewEnabled}
      viewDisabledTitle={pinViewDisabledTitle(viewDisabledReason, t)}
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
      connectionFeedback={null}
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
