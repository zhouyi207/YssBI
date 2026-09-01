import { useCallback, useMemo, useState, type CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import type { GraphContextMenuActions } from "@/features/application/editor";
import {
  isPinPreviewActionAvailable,
  requestAndOpenPinPreview,
} from "@/features/application/editor/requestPinPreview";
import { openPinInspectableView } from "@/features/application/execution/openInspectableResult";
import { executionResultUi } from "@/features/core/execution";
import { useExecutionRead } from "@/features/core/execution/read";
import {
  buildPinViewParams,
  evaluatePinViewState,
  pinViewDisabledTitle,
} from "@/features/core/execution/pinViewTarget";
import { useGraphRead } from "@/features/core/graph/read";
import { useGraphInteractionUi } from "@/features/core/graphInteraction/ui";
import { useRepeatablePinRemovable } from "@/features/core/pin";
import { getPinTypeColor } from "@/features/core/theme/pinTypeTheme";
import { useTheme } from "@/features/core/theme/useTheme";
import type { PinMetaDataDTO } from "@/shared/types/domain";
import type { Pin as PinModel } from "@/shared/types/domain";
import { dataValueFromBackend, dataValueToRaw } from "@/shared/types/domain/dataValue";
import type {
  PortAddressDto,
  PortKindDto,
  ResolvedPortStatusDto,
} from "@/shared/types/domain/editorProjection";
import {
  isExecPin,
  PRIMITIVE_SCALAR_INPUT_KEYS,
  scalarPinInputKey,
} from "@/shared/types/domain/pinSemantics";
import { resolvePinRenderStyle, resolvePinVisualSpec } from "@/shared/types/domain/pinVisual";
import type { PinHistoryProjection } from "@/shared/types/ui";
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

export interface GraphPinControllerProps extends PinModel {
  connected?: boolean;
  linkCount?: number;
  connectionIds?: string[];
  metaData?: PinMetaDataDTO;
  graphPath?: string;
  groupId?: string;
  contextMenuActions?: GraphContextMenuActions | null;
  onPinClick?: (id: string, direction: "input" | "output") => void;
  onPinPointerDown?: (event: React.PointerEvent, pin: PinModel) => void;
  isActive?: boolean;
  pinDragState?: GraphPinDragState;
  onValueChange?: (pinId: string, value: unknown) => void;
  onRemovePin?: (pinId: string) => void;
  forceShowInput?: boolean;
  address?: PortAddressDto;
  kind?: PortKindDto;
  orphan?: boolean;
  status?: ResolvedPortStatusDto;
}

export function GraphPinController(props: GraphPinControllerProps) {
  const {
    id,
    nodeId,
    name,
    type,
    direction,
    connected = false,
    linkCount = 0,
    metaData,
    graphPath,
    groupId,
    contextMenuActions,
    onPinClick,
    onPinPointerDown,
    isActive,
    pinDragState = "normal",
    dataType,
    optional,
    defaultValue,
    userValue,
    onValueChange,
    onRemovePin,
    forceShowInput,
    validationWarning,
    address,
    kind,
    orphan,
    status,
  } = props;
  const { t } = useTranslation();
  const { tokens } = useTheme();
  const isConnected = connected || linkCount > 0 || (isActive ?? false);
  const pinSemantics = useMemo(() => ({ type: type ?? "object", dataType }), [dataType, type]);
  const visualSpec = useMemo(() => resolvePinVisualSpec(pinSemantics), [pinSemantics]);
  const baseColor = getPinTypeColor(visualSpec.colorKey, tokens);
  const renderStyle = useMemo(
    () => resolvePinRenderStyle(visualSpec, isConnected, baseColor, tokens.mutedForeground),
    [baseColor, isConnected, tokens.mutedForeground, visualSpec],
  );

  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);
  const [historyProjection, setHistoryProjection] = useState<PinHistoryProjection>();
  const canRemoveRepeatable = useRepeatablePinRemovable(nodeId, id, graphPath);
  const canRemovePin =
    canRemoveRepeatable && (onRemovePin != null || contextMenuActions?.removeRepeatablePin != null);

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
  const pinHistories = useExecutionRead((snapshot) =>
    graphPath ? snapshot.graphs[graphPath]?.pinHistories : undefined,
  );
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
            isExec: isExecPin(pinSemantics),
            connections,
          })
        : null,
    [address, connections, direction, graphPath, pinSemantics],
  );
  const viewState = useMemo(
    () => (viewParams ? evaluatePinViewState(viewParams) : null),
    [viewParams],
  );
  const previewActionAvailable = isPinPreviewActionAvailable(graphPath, {
    direction,
    kind,
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

  const handleRemovePin = useCallback(() => {
    if (onRemovePin) {
      onRemovePin(id);
      return;
    }
    void contextMenuActions?.removeRepeatablePin(nodeId, id);
  }, [contextMenuActions, id, nodeId, onRemovePin]);

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
    pinHistories,
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
    !optional && !isConnected && direction === "input" && !isExecPin(pinSemantics);
  const isDropdownPin =
    metaData?.showWidget &&
    metaData.widgetType === "dropdown" &&
    (metaData.widgetOptions?.length ?? 0) > 0;
  const showInput = Boolean(
    (!isConnected || forceShowInput === true) &&
    scalarInputKey != null &&
    (PRIMITIVE_SCALAR_INPUT_KEYS.has(scalarInputKey) ||
      (scalarInputKey === "string" && isDropdownPin)) &&
    !visualSpec.container &&
    (direction === "input" || forceShowInput === true) &&
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
    (validationWarning
      ? `${name} (${visualSpec.label}) — ${validationWarning}`
      : `${name} (${visualSpec.label})`);

  const inputSlot = showInput ? (
    <PinInput
      pinId={id}
      nodeId={nodeId}
      graphPath={graphPath!}
      dataType={dataType}
      metaData={metaData}
      value={toDisplayValue(userValue ?? defaultValue)}
      onValueChange={(value) => onValueChange?.(id, value)}
    />
  ) : null;
  const contextMenuSlot = contextMenu ? (
    <PinContextMenu
      position={contextMenu}
      removable={canRemovePin}
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
      onRemove={handleRemovePin}
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
      validationWarning={validationWarning}
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
        onPinPointerDown(event, props);
      }}
      onClick={(event) => {
        event.stopPropagation();
        onPinClick?.(id, direction);
      }}
    />
  );
}
