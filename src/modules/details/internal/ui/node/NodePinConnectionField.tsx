import { useMemo, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { VscAdd, VscRemove } from "react-icons/vsc";

import { Button } from "@/components/ui/button";
import type {
  NodeData,
  PinData,
  ConnectionData,
} from "@/features/domain/editorProjection/graphRuntimeTypes";
import { Select } from "@/shared/ui/Select";
import {
  connectPinsById,
  disconnectConnectionById,
  disconnectPinById,
} from "@/features/application/editor/edgeOperations";

import type { NodePinViewModel } from "./NodePinViewModel";
import { DetailFieldRow } from "../shared/DetailFieldRow";
import {
  connectedPeerId,
  listCompatiblePinOptions,
  listPinConnections,
} from "./nodePinConnectionOptions";
import { graphDraftMutationMessageKey, graphDraftMutationSucceeded } from "./nodeMutationFeedback";

interface NodePinConnectionFieldProps {
  graphPath: string;
  pin: NodePinViewModel;
  pinData: PinData | undefined;
  pins: readonly PinData[];
  nodes: Readonly<Record<string, NodeData>>;
  connections: readonly ConnectionData[];
  disabled?: boolean;
}

interface ConnectionRowProps {
  children: ReactNode;
  removeLabel: string;
  onRemove: () => void;
  showRemove: boolean;
  testId: string;
  disabled: boolean;
}

function ConnectionRow({
  children,
  removeLabel,
  onRemove,
  showRemove,
  testId,
  disabled,
}: ConnectionRowProps) {
  return (
    <div className="flex min-w-0 items-center gap-1">
      <div className="min-w-0 flex-1">{children}</div>
      {showRemove && (
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          aria-label={removeLabel}
          data-testid={testId}
          disabled={disabled}
          onClick={onRemove}
        >
          <VscRemove aria-hidden="true" data-icon="inline-start" />
        </Button>
      )}
    </div>
  );
}

export function NodePinConnectionField({
  graphPath,
  pin,
  pinData,
  pins,
  nodes,
  connections,
  disabled = false,
}: NodePinConnectionFieldProps) {
  const { t } = useTranslation();
  const [emptySlots, setEmptySlots] = useState(0);
  const [busy, setBusy] = useState(false);
  const [errorKey, setErrorKey] = useState<string | null>(null);

  const anchor = pinData;
  const connectionRecords = useMemo(
    () => (anchor ? listPinConnections(anchor.id, anchor.direction, connections) : []),
    [anchor, connections],
  );
  const connectionTargets = useMemo(
    () =>
      new Set(
        connectionRecords
          .map((connection) =>
            anchor ? connectedPeerId(anchor.id, anchor.direction, connection) : null,
          )
          .filter((id): id is string => Boolean(id)),
      ),
    [anchor, connectionRecords],
  );

  const inputOptions = useMemo(() => {
    if (!anchor) return [];
    return listCompatiblePinOptions(anchor, pins, nodes, {
      connections,
      includedIds: connectionTargets,
    });
  }, [anchor, connections, connectionTargets, nodes, pins]);

  const outputOptions = useMemo(() => {
    if (!anchor) return [];
    return listCompatiblePinOptions(anchor, pins, nodes, {
      connections,
      excludedIds: connectionTargets,
    });
  }, [anchor, connections, connectionTargets, nodes, pins]);

  const handleInputChange = async (value: string) => {
    if (!anchor || busy || disabled) return;
    setBusy(true);
    setErrorKey(null);
    const result = value
      ? await connectPinsById(graphPath, value, anchor.id)
      : await disconnectPinById(graphPath, anchor.id);
    setErrorKey(graphDraftMutationMessageKey(result, "detail.nodeDoc.connectionFailed"));
    setBusy(false);
  };

  const handleOutputConnect = async (value: string) => {
    if (!anchor || !value || busy || disabled) return;
    setBusy(true);
    setErrorKey(null);
    const result = await connectPinsById(graphPath, anchor.id, value);
    if (graphDraftMutationSucceeded(result)) setEmptySlots((count) => Math.max(0, count - 1));
    setErrorKey(graphDraftMutationMessageKey(result, "detail.nodeDoc.connectionFailed"));
    setBusy(false);
  };

  const handleOutputDisconnect = async (connectionId: string) => {
    if (busy || disabled) return;
    setBusy(true);
    setErrorKey(null);
    const result = await disconnectConnectionById(graphPath, connectionId);
    setErrorKey(graphDraftMutationMessageKey(result, "detail.nodeDoc.connectionFailed"));
    setBusy(false);
  };

  const pinTitle = pin.name || t("detail.nodeDoc.unnamed");
  if (pin.direction === "input") {
    const currentOutputId =
      connectionRecords[0] && anchor
        ? (connectedPeerId(anchor.id, anchor.direction, connectionRecords[0]) ?? "")
        : "";
    return (
      <div>
        <DetailFieldRow label={pinTitle}>
          <Select
            value={currentOutputId}
            options={[{ value: "", label: t("detail.nodeDoc.unconnected") }, ...inputOptions]}
            onChange={handleInputChange}
            disabled={busy || disabled || !anchor}
            id={`detail-input-${pin.id}`}
          />
        </DetailFieldRow>
        {errorKey && (
          <div role="alert" className="px-1 text-right text-[10px] text-destructive">
            {t(errorKey)}
          </div>
        )}
      </div>
    );
  }

  const pendingCount = Math.max(1 - connectionRecords.length, 0) + emptySlots;
  const outputLabel = (
    <div className="flex min-w-0 items-center justify-between gap-1">
      <span className="min-w-0 truncate">{pinTitle}</span>
      <Button
        type="button"
        variant="ghost"
        size="icon-xs"
        aria-label={t("detail.nodeDoc.addConnection")}
        data-testid={`add-output-connection-${pin.id}`}
        disabled={busy || disabled || outputOptions.length === 0}
        onClick={() => setEmptySlots((count) => count + 1)}
      >
        <VscAdd aria-hidden="true" data-icon="inline-start" />
      </Button>
    </div>
  );
  const connectedOptions = anchor
    ? listCompatiblePinOptions(anchor, pins, nodes, {
        connections,
        includedIds: connectionTargets,
      })
    : [];

  return (
    <div>
      <DetailFieldRow label={outputLabel}>
        <div className="flex min-w-0 flex-col gap-1">
          {connectionRecords.map((connection, index) => {
            const targetId = anchor
              ? (connectedPeerId(anchor.id, anchor.direction, connection) ?? "")
              : "";
            return (
              <ConnectionRow
                key={connection.id}
                removeLabel={t("detail.nodeDoc.removeConnection")}
                onRemove={() => void handleOutputDisconnect(connection.id)}
                showRemove
                testId={`remove-output-connection-${pin.id}-${index}`}
                disabled={busy || disabled}
              >
                <Select
                  value={targetId}
                  options={connectedOptions}
                  onChange={() => undefined}
                  disabled
                  id={`detail-output-${pin.id}-${index}`}
                />
              </ConnectionRow>
            );
          })}
          {Array.from({ length: pendingCount }, (_, index) => (
            <ConnectionRow
              key={`pending-${index}`}
              removeLabel={t("detail.nodeDoc.removeConnection")}
              onRemove={() => setEmptySlots((count) => Math.max(0, count - 1))}
              showRemove={connectionRecords.length + pendingCount > 1}
              testId={`remove-output-connection-${pin.id}-${connectionRecords.length + index}`}
              disabled={busy || disabled}
            >
              <Select
                value=""
                options={[{ value: "", label: t("detail.nodeDoc.selectInput") }, ...outputOptions]}
                onChange={handleOutputConnect}
                disabled={busy || disabled || !anchor}
                id={`detail-output-${pin.id}-${connectionRecords.length + index}`}
              />
            </ConnectionRow>
          ))}
        </div>
      </DetailFieldRow>
      {errorKey && (
        <div role="alert" className="px-1 text-right text-[10px] text-destructive">
          {t(errorKey)}
        </div>
      )}
    </div>
  );
}
