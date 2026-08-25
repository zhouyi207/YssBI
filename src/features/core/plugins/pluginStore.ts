import { create } from 'zustand';

export const PLUGIN_INSTALLATION_STORAGE_KEY = 'yssbi-installed-plugins-v1';

interface PluginStore {
  readonly installedPluginIds: readonly string[];
  installPlugin: (pluginId: string) => void;
  uninstallPlugin: (pluginId: string) => void;
}

function readInstalledPluginIds(): string[] {
  if (typeof localStorage === 'undefined') {
    return [];
  }

  try {
    const stored = localStorage.getItem(PLUGIN_INSTALLATION_STORAGE_KEY);
    if (!stored) {
      return [];
    }

    const parsed: unknown = JSON.parse(stored);
    if (!Array.isArray(parsed)) {
      return [];
    }

    return [...new Set(parsed.filter((value): value is string => typeof value === 'string' && value.length > 0))];
  } catch {
    return [];
  }
}

function persistInstalledPluginIds(pluginIds: readonly string[]): void {
  if (typeof localStorage === 'undefined') {
    return;
  }

  try {
    localStorage.setItem(PLUGIN_INSTALLATION_STORAGE_KEY, JSON.stringify(pluginIds));
  } catch {
    // A non-persistent preference should not make plugin actions fail.
  }
}

export const usePluginStore = create<PluginStore>((set) => ({
  installedPluginIds: readInstalledPluginIds(),
  installPlugin: (pluginId) => {
    set((state) => {
      if (state.installedPluginIds.includes(pluginId)) {
        return state;
      }

      const installedPluginIds = [...state.installedPluginIds, pluginId];
      persistInstalledPluginIds(installedPluginIds);
      return { installedPluginIds };
    });
  },
  uninstallPlugin: (pluginId) => {
    set((state) => {
      const installedPluginIds = state.installedPluginIds.filter((id) => id !== pluginId);
      if (installedPluginIds.length === state.installedPluginIds.length) {
        return state;
      }

      persistInstalledPluginIds(installedPluginIds);
      return { installedPluginIds };
    });
  },
}));
