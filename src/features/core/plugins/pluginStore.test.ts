// @vitest-environment happy-dom

import { beforeEach, describe, expect, it } from 'vitest';

import {
  PLUGIN_INSTALLATION_STORAGE_KEY,
  usePluginStore,
} from './pluginStore';

describe('pluginStore', () => {
  beforeEach(() => {
    localStorage.clear();
    usePluginStore.setState({ installedPluginIds: [] });
  });

  it('persists installation and removes the slot when the plugin is uninstalled', () => {
    usePluginStore.getState().installPlugin('julia');

    expect(usePluginStore.getState().installedPluginIds).toEqual(['julia']);
    expect(localStorage.getItem(PLUGIN_INSTALLATION_STORAGE_KEY)).toBe('["julia"]');

    usePluginStore.getState().uninstallPlugin('julia');

    expect(usePluginStore.getState().installedPluginIds).toEqual([]);
    expect(localStorage.getItem(PLUGIN_INSTALLATION_STORAGE_KEY)).toBe('[]');
  });
});
