import { useSyncExternalStore } from 'react';

import type { DeepReadonly } from '@/shared/types/deepReadonly';
import { usePluginStore } from './pluginStore';

export interface PluginUiSnapshot {
  readonly installedPluginIds: readonly string[];
}

export interface PluginUiCapability {
  readonly getSnapshot: () => DeepReadonly<PluginUiSnapshot>;
  readonly subscribe: (listener: () => void) => () => void;
  readonly installPlugin: (pluginId: string) => void;
  readonly uninstallPlugin: (pluginId: string) => void;
}

function snapshot(): DeepReadonly<PluginUiSnapshot> {
  const state = usePluginStore.getState();
  return Object.freeze({
    installedPluginIds: Object.freeze([...state.installedPluginIds]),
  });
}

let currentSnapshot = snapshot();
const listeners = new Set<() => void>();

usePluginStore.subscribe(() => {
  currentSnapshot = snapshot();
  for (const listener of listeners) listener();
});

export function getPluginUiSnapshot(): DeepReadonly<PluginUiSnapshot> {
  return currentSnapshot;
}

export function subscribePluginUi(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function usePluginUi<T>(
  selector: (state: DeepReadonly<PluginUiSnapshot>) => T,
): T {
  const state = useSyncExternalStore(subscribePluginUi, getPluginUiSnapshot, getPluginUiSnapshot);
  return selector(state);
}

export const pluginUi: PluginUiCapability = {
  getSnapshot: getPluginUiSnapshot,
  subscribe: subscribePluginUi,
  installPlugin: (pluginId) => usePluginStore.getState().installPlugin(pluginId),
  uninstallPlugin: (pluginId) => usePluginStore.getState().uninstallPlugin(pluginId),
};
