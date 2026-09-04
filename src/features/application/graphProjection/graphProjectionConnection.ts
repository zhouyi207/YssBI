const reconnectListeners = new Set<() => void>();

export function requestGraphProjectionReconnect(): void {
  for (const listener of reconnectListeners) listener();
}

export function subscribeGraphProjectionReconnect(listener: () => void): () => void {
  reconnectListeners.add(listener);
  return () => reconnectListeners.delete(listener);
}
