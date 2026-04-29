import { useSyncExternalStore } from "react";
import { uiStore } from "./UIStore";

type UIState = ReturnType<typeof uiStore.getState>;

export function useUIStore<T>(selector: (state: UIState) => T): T;
export function useUIStore<T>(selector: (state: UIState) => T): T {
  return useSyncExternalStore(
    uiStore.subscribe.bind(uiStore),
    () => selector(uiStore.getState()),
    () => selector(uiStore.getState())
  );
}
