import { useSyncExternalStore } from "react";

import type { DeepReadonly } from "@/shared/types/deepReadonly";
import { useModifierKeyStore } from "./useModifierKeyStore";

export interface KeyboardUiSnapshot {
  readonly altKey: boolean;
  readonly ctrlKey: boolean;
  readonly shiftKey: boolean;
}

export interface KeyboardUiCapability {
  readonly getSnapshot: () => DeepReadonly<KeyboardUiSnapshot>;
  readonly subscribe: (listener: () => void) => () => void;
  readonly setModifierKeys: (keys: KeyboardUiSnapshot) => void;
  readonly resetModifierKeys: () => void;
}

function buildSnapshot(): DeepReadonly<KeyboardUiSnapshot> {
  const state = useModifierKeyStore.getState();
  return Object.freeze({
    altKey: state.altKey,
    ctrlKey: state.ctrlKey,
    shiftKey: state.shiftKey,
  });
}

let currentSnapshot = buildSnapshot();
const listeners = new Set<() => void>();

function refreshSnapshot(): void {
  currentSnapshot = buildSnapshot();
  for (const listener of listeners) listener();
}

useModifierKeyStore.subscribe(refreshSnapshot);

export function getKeyboardUiSnapshot(): DeepReadonly<KeyboardUiSnapshot> {
  return currentSnapshot;
}

export function subscribeKeyboardUi(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useKeyboardUi<T>(selector: (snapshot: DeepReadonly<KeyboardUiSnapshot>) => T): T {
  const snapshot = useSyncExternalStore(
    subscribeKeyboardUi,
    getKeyboardUiSnapshot,
    getKeyboardUiSnapshot,
  );
  return selector(snapshot);
}

export const keyboardUi: KeyboardUiCapability = {
  getSnapshot: getKeyboardUiSnapshot,
  subscribe: subscribeKeyboardUi,
  setModifierKeys: (keys) => useModifierKeyStore.getState().setModifierKeys(keys),
  resetModifierKeys: () => useModifierKeyStore.getState().resetModifierKeys(),
};
