import { useSyncExternalStore } from "react";

import type { DeepReadonly } from "@/shared/types/deepReadonly";
import { freezeProjectionSnapshot } from "@/shared/types/deepReadonly";
import { useSettingsStore } from "./settingsStore";
import type {
  AppSettings,
  AppearanceSettings,
  EditorSettings,
  ProjectSettings,
  ThemeSettings,
} from "@/shared/types/settings";

export interface SettingsReadSnapshot {
  readonly theme: DeepReadonly<ThemeSettings>;
  readonly editor: DeepReadonly<EditorSettings>;
  readonly appearance: DeepReadonly<AppearanceSettings>;
  readonly project: DeepReadonly<ProjectSettings>;
  readonly isLoading: boolean;
}

export interface SettingsReadCapability {
  readonly getSnapshot: () => DeepReadonly<SettingsReadSnapshot>;
  readonly subscribe: (listener: () => void) => () => void;
}

function buildSnapshot(): DeepReadonly<SettingsReadSnapshot> {
  const state = useSettingsStore.getState();
  const snapshot: SettingsReadSnapshot = {
    theme: state.theme,
    editor: state.editor,
    appearance: state.appearance,
    project: state.project,
    isLoading: state.isLoading,
  };
  return freezeProjectionSnapshot(snapshot);
}

let currentSnapshot = buildSnapshot();
const listeners = new Set<() => void>();

function refreshSnapshot(): void {
  currentSnapshot = buildSnapshot();
  for (const listener of listeners) listener();
}

useSettingsStore.subscribe(refreshSnapshot);

export function getSettingsSnapshot(): DeepReadonly<SettingsReadSnapshot> {
  return currentSnapshot;
}

export function subscribeSettingsRead(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useSettingsRead<T>(
  selector: (snapshot: DeepReadonly<SettingsReadSnapshot>) => T,
): T {
  const snapshot = useSyncExternalStore(
    subscribeSettingsRead,
    getSettingsSnapshot,
    getSettingsSnapshot,
  );
  return selector(snapshot);
}

export const settingsRead: SettingsReadCapability = {
  getSnapshot: getSettingsSnapshot,
  subscribe: subscribeSettingsRead,
};

export type { AppSettings };
