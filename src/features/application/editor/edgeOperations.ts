import { executeCommand } from '@/features/core/history';
import type { GraphMutationCommandResult } from '@/features/core/history/types';
import { executeSafeGraphMutationOutcome } from '@/features/application/editorMutation/safeGraphMutation';

function isNonEmptyId(value: string): boolean {
  return value.trim().length > 0;
}

export async function disconnectConnectionsById(
  graphPath: string,
  connectionIds: readonly string[],
): Promise<boolean> {
  if (connectionIds.length === 0 || connectionIds.some((id) => !isNonEmptyId(id))) return false;

  return executeCommand(graphPath, 'DisconnectConnections', {
    connectionIds: [...new Set(connectionIds)],
  });
}

export async function insertRerouteAtConnection(
  graphPath: string,
  connectionId: string,
  position: Readonly<{ x: number; y: number }>,
): Promise<GraphMutationCommandResult> {
  if (!isNonEmptyId(connectionId)
    || !Number.isFinite(position.x)
    || !Number.isFinite(position.y)) return false;

  return executeSafeGraphMutationOutcome(
    graphPath,
    'Insert reroute',
    'InsertReroute',
    {
      connectionId,
      position: { x: position.x, y: position.y },
    },
  );
}
