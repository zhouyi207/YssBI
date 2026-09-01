import { useSyncExternalStore } from "react";

interface DockviewSnapshotPort<TSnapshot> {
  subscribe(listener: () => void): () => void;
  getSnapshot(): TSnapshot;
}

export function useDockviewPortSnapshot<TSnapshot>(
  port: DockviewSnapshotPort<TSnapshot>,
): TSnapshot {
  return useSyncExternalStore(port.subscribe, port.getSnapshot, port.getSnapshot);
}
