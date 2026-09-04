import { useCallback, useMemo, useState, type CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import type { GraphContextMenuActions } from "@/features/application/editor";
import {
  isPinPreviewActionAvailable,
  requestAndOpenPinPreview,
} from "@/features/application/editor/requestPinPreview";
import { openPinInspectableView } from "@/features/application/execution/openInspectableResult";
import { executionResultUi } from "@/features/core/execution";
import {
  buildPinViewParams,
  evaluatePinViewState,
  pinViewDisabledTitle,
} from "@/features/core/execution/pinViewTarget";
import { useGraphRead } from "@/features/core/graph/read";
import { useGraphInteractionUi } from "@/features/core/graphInteraction/ui";
import { getPinTypeColor } from "@/features/core/theme/pinTypeTheme";
import { useTheme } from "@/features/core/theme/useTheme";
import type { PinData, PinView } from "@/features/domain/editorProjection/graphRuntimeTypes";
import {
  findPrimaryPortDiagnostic,
  isUnboundInputDiagnostic,
} from "@/features/domain/graphDiagnostics/nodeDiagnostics";
import { dataValueFromBackend, dataValueToRaw } from "@/shared/types/domain/dataValue";
import { PRIMITIVE_SCALAR_INPUT_KEYS, scalarPinInputKey } from "@/shared/types/domain/pinSemantics";
import { resolvePinRenderStyle, resolvePinVisualSpec } from "@/shared/types/domain/pinVisual";
import type { PinHistoryProjection } from "@/features/core/execution/executionTypes";
import { PinContextMenu } from "../ContextMenu";
import { GraphPinView, type GraphPinConnectionFeedbackViewModel } from "./GraphPinView";
import { PinInput } from "./PinInput";

function toDisplayValue(value: unknown): unknown {
  if (value == null) return value;
  if (
    typeof value === "object" &&
    !Array.isArray(value) &&
    ("String" in value ||
      "Boolean" in value ||
      "Int64" in value ||
      "Float64" in value ||
      "Null" in value)
  ) {
    return dataValueToRaw(
      dataValueFromBackend(value as Parameters<typeof dataValueFromBackend>[0]),
    );
  }
  return value;
}

export type GraphPinDragState = "normal" | "highlighted" | "dimmed";

const EMPTY_CONNECTION_IDS: string[] = [];

export interface GraphPinControllerProps {
  pin: PinData & Partial<Pick<PinView, "connected" | "linkCount">>;
  graphPath?: string;
  groupId?: string;
  contextMenuActions?: GraphContextMenuActions | null;
  onPinPointerDown?: (event: React.PointerEvent, pin: PinData) => void;
  isActive?: boolean;
  pinDragState?: GraphPinDragState;
}

export function GraphPinController(props: GraphPinControllerProps) {
  const {
    pin,
    graphPath,
    groupId,
    contextMenuActions,
    onPinPointerDown,
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
  const { t } = useTranslation();
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
  const [historyProjection, setHistoryProjection] = useState<PinHistoryProjection>();

  const connectionFeedback = useGraphInteractionUi((state) => {
    if (!graphPath || !groupId) return null;
    const interaction = state.interactions[graphPath];
    if (!interaction || interaction.type === "idle" || interaction.session.groupId !== groupId) {
      return null;
    }
    if (interaction.type !== "drawingConnection" && interaction.type !== "movingConnections") {
      return null;
    }
    const session = interaction.session;
    return session.snappedTarget?.id === id || session.hoveredTarget?.id === id
      ? session.feedback
      : null;
  });
  const connectionFeedbackModel: GraphPinConnectionFeedbackViewModel | null = connectionFeedback
    ? connectionFeedback.kind === "invalid"
      ? { kind: "invalid", invalidReason: connectionFeedback.reason }
      : { kind: connectionFeedback.kind }
    : null;
  const connectionIds = useGraphRead((snapshot) =>
    graphPath
      ? (snapshot.graphEntities[graphPath]?.pinConnections[id] ?? EMPTY_CONNECTION_IDS)
      : EMPTY_CONNECTION_IDS,
  );
  const graphConnections = useGraphRead((snapshot) =>
    graphPath ? snapshot.graphEntities[graphPath]?.connections : undefined,
  );
  const pinDiagnostic = useGraphRead((snapshot) => {
    const diagnostics = graphPath
      ? snapshot.graphEntities[graphPath]?.nodes[nodeId]?.diagnostics
      : undefined;
    return diagnostics ? findPrimaryPortDiagnostic(diagnostics, address) : undefined;
  });
  const connections = useMemo(
    () =>
      connectionIds.flatMap((connectionId) => {
        const connection = graphConnections?.[connectionId];
        return connection?.output && connection.input
          ? [
              {
                connectionId: connection.id,
                output: connection.output,
                input: connection.input,
                order: connection.order ?? null,
              },
            ]
          : [];
      }),
    [connectionIds, graphConnections],
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
  const viewState = useMemo(
    () => (viewParams ? evaluatePinViewState(viewParams) : null),
    [viewParams],
  );
  const previewActionAvailable = isPinPreviewActionAvailable(graphPath, {
    direction,
    address,
    orphan,
    status,
  });
  const showViewMenu = (viewState?.showMenu ?? false) || previewActionAvailable;
  const viewEnabled = (viewState?.enabled ?? false) || previewActionAvailable;
  const viewDisabledReason = previewActionAvailable ? null : (viewState?.disabledReason ?? null);
  const historyOutputs = useMemo(
    () => viewState?.refs.flatMap((ref) => (ref.kind === "outputPin" ? [ref.output] : [])) ?? [],
    [viewState],
  );
  const firstHistoryOutput = historyOutputs[0];

  const handleView = useCallback(() => {
    if (!viewParams || !graphPath) return;
    if (viewState?.enabled) {
      void openPinInspectableView(viewParams, t).then(() => {
        if (!firstHistoryOutput) return;
        const history = executionResultUi.getPinHistory(graphPath, firstHistoryOutput);
        setHistoryProjection(
          history ? (structuredClone(history) as unknown as PinHistoryProjection) : undefined,
        );
      });
      return;
    }
    if (previewActionAvailable) {
      void requestAndOpenPinPreview(graphPath, id, t);
    }
  }, [
    firstHistoryOutput,
    graphPath,
    id,
    previewActionAvailable,
    t,
    viewParams,
    viewState?.enabled,
  ]);

  const hasLinks = linkCount > 0 || connectionIds.length > 0;
  const scalarInputKey = scalarPinInputKey(dataType);
  const canReset =
    direction === "input" &&
    scalarInputKey != null &&
    PRIMITIVE_SCALAR_INPUT_KEYS.has(scalarInputKey) &&
    !visualSpec.container &&
    userValue != null;
  const shouldPulse =
    !isConnected && direction === "input" && isUnboundInputDiagnostic(pinDiagnostic);
  const showInput = Boolean(
    !isConnected &&
    scalarInputKey != null &&
    PRIMITIVE_SCALAR_INPUT_KEYS.has(scalarInputKey) &&
    !visualSpec.container &&
    direction === "input" &&
    graphPath &&
    nodeId,
  );
  const effectivePinDragState = contextMenu ? "highlighted" : pinDragState;
  const dragStyle: CSSProperties | undefined =
    effectivePinDragState === "dimmed"
      ? { opacity: 0.25, transition: "opacity 150ms, filter 150ms" }
      : effectivePinDragState === "highlighted"
        ? { filter: "brightness(1.25) saturate(1.4)", transition: "opacity 150ms, filter 150ms" }
        : undefined;
  const feedbackTooltip =
    connectionFeedback?.kind === "invalid"
      ? t(`canvas.connection.feedback.${connectionFeedback.reason}`)
      : null;
  const tooltip =
    feedbackTooltip ??
    (pinDiagnostic
      ? `${name} (${visualSpec.label}) — ${pinDiagnostic.message}`
      : `${name} (${visualSpec.label})`);

  const inputSlot = showInput ? (
    <PinInput
      pinId={id}
      nodeId={nodeId}
      graphPath={graphPath!}
      dataType={dataType}
      value={toDisplayValue(userValue ?? defaultValue)}
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
      historyEntries={historyProjection?.entries}
      onViewHistory={(resultId) => {
        if (!historyProjection || !viewParams) return;
        executionResultUi.recordPinHistory({
          ...historyProjection,
          selectedResultId: resultId,
        });
        void openPinInspectableView(viewParams, t, { selectedResultId: resultId });
      }}
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
      diagnosticMessage={pinDiagnostic?.message}
      dragStyle={dragStyle}
      connectionFeedback={connectionFeedbackModel}
      visualSpec={visualSpec}
      renderStyle={renderStyle}
      baseColor={baseColor}
      shouldPulse={shouldPulse}
      tooltip={tooltip}
      inputSlot={inputSlot}
      contextMenuSlot={contextMenuSlot}
      onContextMenu={(event) => {
        event.preventDefault();
        event.stopPropagation();
        setContextMenu({ x: event.clientX, y: event.clientY });
      }}
      onPointerDown={(event) => {
        if (!onPinPointerDown) return;
        event.stopPropagation();
        event.preventDefault();
        onPinPointerDown(event, pin);
      }}
      onClick={(event) => {
        event.stopPropagation();
      }}
    />
  );
}
