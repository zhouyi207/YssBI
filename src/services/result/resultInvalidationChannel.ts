const CHANNEL_NAME = "yssbi-current-results";

/** Null invalidates every result when the project session is replaced. */
export function publishResultInvalidation(resultIds: readonly string[] | null): void {
  if (typeof BroadcastChannel === "undefined") return;
  const channel = new BroadcastChannel(CHANNEL_NAME);
  channel.postMessage(resultIds);
  channel.close();
}

export function subscribeResultInvalidation(
  listener: (resultIds: readonly string[] | null) => void,
): () => void {
  if (typeof BroadcastChannel === "undefined") return () => {};
  const channel = new BroadcastChannel(CHANNEL_NAME);
  channel.onmessage = (event: MessageEvent<unknown>) => {
    if (
      event.data === null ||
      (Array.isArray(event.data) && event.data.every((id) => typeof id === "string"))
    )
      listener(event.data);
  };
  return () => channel.close();
}
