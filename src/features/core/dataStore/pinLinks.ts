/**
 * Runtime pin connection state is derived from `pinConnections`.
 * Store pins never persist peer pin ids — only connection ids.
 */
export function otherEndpointFromConnectionId(connectionId: string, pinId: string): string {
  const sep = connectionId.indexOf('->');
  if (sep < 0) return connectionId;
  const from = connectionId.slice(0, sep);
  const to = connectionId.slice(sep + 2);
  return from === pinId ? to : from;
}

export function derivePinLinks(pinId: string, connectionIds: readonly string[] | undefined): string[] {
  return (connectionIds ?? []).map((connectionId) =>
    otherEndpointFromConnectionId(connectionId, pinId),
  );
}

export function derivePinConnectionView(connectionIds: readonly string[] | undefined): {
  connected: boolean;
  linkCount: number;
  connectionIds: string[];
} {
  const ids = connectionIds ? [...connectionIds] : [];
  return {
    connected: ids.length > 0,
    linkCount: ids.length,
    connectionIds: ids,
  };
}
