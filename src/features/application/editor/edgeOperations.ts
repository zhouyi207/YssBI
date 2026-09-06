import { executeGraphEdit } from "@/features/application/graphEditing";
import type { GraphEditOutcome } from "@/features/application/graphEditing/types";

function isNonEmptyId(value: string): boolean {
  return value.trim().length > 0;
}

export async function connectPinsById(
  graphPath: string,
  pinA: string,
  pinB: string,
): Promise<GraphEditOutcome> {
  if (!isNonEmptyId(pinA) || !isNonEmptyId(pinB)) return { status: "unavailable" };

  return executeGraphEdit(graphPath, "ConnectPins", {
    pinA,
    pinB,
  });
}

export async function disconnectConnectionById(
  graphPath: string,
  connectionId: string,
): Promise<GraphEditOutcome> {
  if (!isNonEmptyId(connectionId)) return { status: "unavailable" };

  return executeGraphEdit(graphPath, "DisconnectConnections", { connectionIds: [connectionId] });
}

export async function disconnectPinById(
  graphPath: string,
  pinId: string,
): Promise<GraphEditOutcome> {
  if (!isNonEmptyId(pinId)) return { status: "unavailable" };

  return executeGraphEdit(graphPath, "DisconnectPort", {
    pinId,
  });
}

export async function disconnectConnectionsById(
  graphPath: string,
  connectionIds: readonly string[],
): Promise<boolean> {
  if (connectionIds.length === 0 || connectionIds.some((id) => !isNonEmptyId(id))) return false;

  return (
    (
      await executeGraphEdit(graphPath, "DisconnectConnections", {
        connectionIds: [...new Set(connectionIds)],
      })
    ).status === "applied"
  );
}

export async function insertRerouteAtConnection(
  graphPath: string,
  connectionId: string,
  position: Readonly<{ x: number; y: number }>,
): Promise<GraphEditOutcome> {
  if (!isNonEmptyId(connectionId) || !Number.isFinite(position.x) || !Number.isFinite(position.y))
    return { status: "unavailable" };

  return executeGraphEdit(graphPath, "InsertReroute", {
    connectionId,
    position: { x: position.x, y: position.y },
  });
}
