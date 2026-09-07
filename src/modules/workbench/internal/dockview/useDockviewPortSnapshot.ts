import { useSyncExternalStore } from "react";

interface DockviewSnapshotPort<TSnapshot> {
  subscribe(listener: () => void): () => void;
  getSnapshot(): TSnapshot;
}

export function useDockviewPortSnapshot<TSnapshot>(
  port: DockviewSnapshotPort<TSnapshot>,
): TSnapshot;
export function useDockviewPortSnapshot<TSnapshot, TSelected>(
  port: DockviewSnapshotPort<TSnapshot>,
  select: (snapshot: TSnapshot) => TSelected,
): TSelected;
export function useDockviewPortSnapshot<TSnapshot, TSelected>(
  port: DockviewSnapshotPort<TSnapshot>,
  select?: (snapshot: TSnapshot) => TSelected,
): TSnapshot | TSelected {
  // Selectors must return a stable value while their observed fields are unchanged.
  const getSnapshot = select ? () => select(port.getSnapshot()) : port.getSnapshot;
  return useSyncExternalStore<TSnapshot | TSelected>(port.subscribe, getSnapshot, getSnapshot);
}
