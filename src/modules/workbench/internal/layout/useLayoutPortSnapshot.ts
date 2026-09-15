import { useSyncExternalStore } from "react";

interface FlexLayoutSnapshotPort<TSnapshot> {
  subscribe(listener: () => void): () => void;
  getSnapshot(): TSnapshot;
}

export function useLayoutPortSnapshot<TSnapshot>(
  port: FlexLayoutSnapshotPort<TSnapshot>,
): TSnapshot;
export function useLayoutPortSnapshot<TSnapshot, TSelected>(
  port: FlexLayoutSnapshotPort<TSnapshot>,
  select: (snapshot: TSnapshot) => TSelected,
): TSelected;
export function useLayoutPortSnapshot<TSnapshot, TSelected>(
  port: FlexLayoutSnapshotPort<TSnapshot>,
  select?: (snapshot: TSnapshot) => TSelected,
): TSnapshot | TSelected {
  // Selectors must return a stable value while their observed fields are unchanged.
  const getSnapshot = select ? () => select(port.getSnapshot()) : port.getSnapshot;
  return useSyncExternalStore<TSnapshot | TSelected>(port.subscribe, getSnapshot, getSnapshot);
}
