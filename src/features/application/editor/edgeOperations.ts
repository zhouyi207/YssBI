import { executeCommand } from "@/features/core/history";
import type { GraphMutationCommandResult } from "@/features/core/history/types";
import { executeSafeGraphMutationOutcome } from "@/features/application/editorMutation/safeGraphMutation";
import { ensureGraphMutationPortRegistered } from "@/features/application/editorMutation/registerGraphMutationPort";

function isNonEmptyId(value: string): boolean {
  return value.trim().length > 0;
}

export async function connectPinsById(
  graphPath: string,
  pinA: string,
  pinB: string,
): Promise<GraphMutationCommandResult> {
  if (!isNonEmptyId(pinA) || !isNonEmptyId(pinB)) return false;
  ensureGraphMutationPortRegistered();
  return executeSafeGraphMutationOutcome(graphPath, "Detail connect pins", "ConnectPins", {
    pinA,
    pinB,
  });
}

export async function disconnectConnectionById(
  graphPath: string,
  connectionId: string,
): Promise<GraphMutationCommandResult> {
  if (!isNonEmptyId(connectionId)) return false;
  ensureGraphMutationPortRegistered();
  return executeSafeGraphMutationOutcome(
    graphPath,
    "Detail disconnect connection",
    "DisconnectConnections",
    { connectionIds: [connectionId] },
  );
}

export async function disconnectPinById(
  graphPath: string,
  pinId: string,
): Promise<GraphMutationCommandResult> {
  if (!isNonEmptyId(pinId)) return false;
  ensureGraphMutationPortRegistered();
  return executeSafeGraphMutationOutcome(graphPath, "Detail disconnect port", "DisconnectPort", {
    pinId,
  });
}

export async function disconnectConnectionsById(
  graphPath: string,
  connectionIds: readonly string[],
): Promise<boolean> {
  if (connectionIds.length === 0 || connectionIds.some((id) => !isNonEmptyId(id))) return false;

  return executeCommand(graphPath, "DisconnectConnections", {
    connectionIds: [...new Set(connectionIds)],
  });
}

export async function insertRerouteAtConnection(
  graphPath: string,
  connectionId: string,
  position: Readonly<{ x: number; y: number }>,
): Promise<GraphMutationCommandResult> {
  if (!isNonEmptyId(connectionId) || !Number.isFinite(position.x) || !Number.isFinite(position.y))
    return false;

  return executeSafeGraphMutationOutcome(graphPath, "Insert reroute", "InsertReroute", {
    connectionId,
    position: { x: position.x, y: position.y },
  });
}
