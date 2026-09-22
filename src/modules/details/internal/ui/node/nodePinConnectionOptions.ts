import type { DeepReadonly } from "@/shared/types/deepReadonly";
import { resolveConnectionCompatibility } from "@/features/domain/editorProjection/connectionRules";
import type { ConnectionData, PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import { formatNodePinDisplayLabel, pinDisplayTitle } from "@/features/domain/editorProjection";

export interface PinConnectionOption {
  label: string;
  value: string;
}

export interface PinConnectionOptionConfig {
  connections?: DeepReadonly<ConnectionData[]>;
  excludedIds?: ReadonlySet<string>;
  includedIds?: ReadonlySet<string>;
}

export function formatPinConnectionOptionLabel(
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

function candidateCanAppendToInput(
  anchor: DeepReadonly<PinData>,
  candidate: DeepReadonly<PinData>,
  connections: DeepReadonly<ConnectionData[]>,
): boolean {
  if (anchor.direction !== "output" || candidate.direction !== "input") return true;
  if (candidate.connections?.canAppend) return true;
  return !listPinConnections(candidate.id, "input", connections).some(
    (connection) => connection.from !== anchor.id,
  );
}

export function listCompatiblePinOptions(
  anchor: DeepReadonly<PinData>,
  pins: DeepReadonly<PinData[]>,
  nodeTitles: Readonly<Record<string, string>>,
  {
    connections = [],
    excludedIds = new Set<string>(),
    includedIds = new Set<string>(),
  }: PinConnectionOptionConfig = {},
): PinConnectionOption[] {
  return pins
    .filter((candidate) => candidate.direction !== anchor.direction)
    .filter((candidate) => !excludedIds.has(candidate.id) || includedIds.has(candidate.id))
    .filter((candidate) => {
      if (includedIds.has(candidate.id)) return true;
      if (!candidateCanAppendToInput(anchor, candidate, connections)) return false;
      const output = anchor.direction === "output" ? anchor : candidate;
      const input = anchor.direction === "input" ? anchor : candidate;
      return resolveConnectionCompatibility(output, input).kind !== "invalid";
    })
    .map((candidate) => ({
      value: candidate.id,
      label: formatPinConnectionOptionLabel(candidate, nodeTitles),
    }));
}
