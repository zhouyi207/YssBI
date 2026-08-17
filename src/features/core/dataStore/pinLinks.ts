/**
 * Runtime pin connection state is derived from `pinConnections`.
 * Store pins never persist peer pin ids — only connection ids.
 */
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
