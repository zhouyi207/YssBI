import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type { ConnectionDecision } from "@/shared/types/domain/connectionCandidates";
import type { ConnectionData, PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import { formatNodePinDisplayLabel, pinDisplayTitle } from "@/features/domain/editorProjection";

export interface PinConnectionOption {
  label: string;
  value: string;
}

function formatPinConnectionOptionLabel(
  pin: DeepReadonly<PinData>,
  nodeTitles: Readonly<Record<string, string>>,
): string {
  return formatNodePinDisplayLabel(nodeTitles[pin.nodeId], pinDisplayTitle(pin)) ?? "";
}

export function listPinConnections(
  pinId: string,
  direction: PinData["direction"],
  connections: DeepReadonly<ConnectionData[]>,
): DeepReadonly<ConnectionData>[] {
  return connections.filter((connection) =>
    direction === "output" ? connection.from === pinId : connection.to === pinId,
  );
}

export function connectedPeerId(
  pinId: string,
  direction: PinData["direction"],
  connection: DeepReadonly<ConnectionData>,
): string | null {
  if (direction === "output") return connection.from === pinId ? connection.to : null;
  return connection.to === pinId ? connection.from : null;
}

export function projectPinConnectionOptions(
  decisions: Readonly<Partial<Record<string, ConnectionDecision>>>,
  pins: DeepReadonly<PinData[]>,
  nodeTitles: Readonly<Record<string, string>>,
  replacementLabel: string,
  selectedIds?: ReadonlySet<string>,
): PinConnectionOption[] {
  return pins
    .filter((candidate) => {
      if (selectedIds?.has(candidate.id)) return true;
      const decision = decisions[candidate.id];
      return decision?.kind === "append" || decision?.kind === "replace";
    })
    .map((candidate) => {
      const label = formatPinConnectionOptionLabel(candidate, nodeTitles);
      return {
        value: candidate.id,
        label:
          decisions[candidate.id]?.kind === "replace" && !selectedIds?.has(candidate.id)
            ? `${label} (${replacementLabel})`
            : label,
      };
    });
}
