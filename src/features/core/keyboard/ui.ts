import { createReadProjection, useReadProjection } from "@/features/core/state/readProjection";

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
  return {
    altKey: state.altKey,
    ctrlKey: state.ctrlKey,
    shiftKey: state.shiftKey,
  };
}

const projection = createReadProjection(buildSnapshot, [useModifierKeyStore]);
export const getKeyboardUiSnapshot = projection.getSnapshot;
export const subscribeKeyboardUi = projection.subscribe;
export function useKeyboardUi<T>(selector: (snapshot: DeepReadonly<KeyboardUiSnapshot>) => T): T {
  return useReadProjection(projection, selector);
}

export const keyboardUi: KeyboardUiCapability = {
  getSnapshot: getKeyboardUiSnapshot,
  subscribe: subscribeKeyboardUi,
  setModifierKeys: (keys) => useModifierKeyStore.getState().setModifierKeys(keys),
  resetModifierKeys: () => useModifierKeyStore.getState().resetModifierKeys(),
};
