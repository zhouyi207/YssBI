import { useSyncExternalStore } from "react";
import { uiStore } from "./UIStore";

type UIState = ReturnType<typeof uiStore.getState>;

export function useUIStore(): UIState;
export function useUIStore<T>(selector: (state: UIState) => T): T;
export function useUIStore<T>(selector?: (state: UIState) => T): T | UIState {
  return useSyncExternalStore(
    uiStore.subscribe.bind(uiStore),
    () => (selector ? selector(uiStore.getState()) : uiStore.getState()) as T | UIState,
    () => (selector ? selector(uiStore.getState()) : uiStore.getState()) as T | UIState
  );
}
