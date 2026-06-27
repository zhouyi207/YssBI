/**
 * Runtime pin links are derived from `pinConnections`.
 * `Pin.links` is a view field and must not become a second source of truth.
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
