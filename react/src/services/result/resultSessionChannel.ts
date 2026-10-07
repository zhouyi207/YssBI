const CHANNEL_NAME = "yssbi-result-sessions";

/** Result snapshots survive output changes; only session teardown closes their views. */
export function publishResultSessionEnd(executionSessionId: string | null): void {
  if (typeof BroadcastChannel === "undefined") return;
  const channel = new BroadcastChannel(CHANNEL_NAME);
  channel.postMessage({ executionSessionId });
  channel.close();
}

export function subscribeResultSessionEnd(
  listener: (executionSessionId: string | null) => void,
): () => void {
  if (typeof BroadcastChannel === "undefined") return () => {};
  const channel = new BroadcastChannel(CHANNEL_NAME);
  channel.onmessage = (event: MessageEvent<unknown>) => {
    const value = event.data;
    if (
      typeof value === "object" &&
      value !== null &&
      "executionSessionId" in value &&
      (value.executionSessionId === null || typeof value.executionSessionId === "string")
    )
      listener(value.executionSessionId);
  };
  return () => channel.close();
}
