import { useSyncExternalStore } from "react";
import { uiStore } from "./UIStore";

export const useUIStore = () => {
  return useSyncExternalStore(
    uiStore.subscribe.bind(uiStore),
    () => uiStore.getState()
  );
};
