// @vitest-environment happy-dom

import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { TFunction } from 'i18next';

import { usePluginStore } from '@/features/core/plugins/pluginStore';

const runtime = vi.hoisted(() => ({
  install: vi.fn(),
}));
const ui = vi.hoisted(() => ({
  confirm: vi.fn(),
  alert: vi.fn(),
  startProgress: vi.fn(),
  finishProgress: vi.fn(),
}));

vi.mock('@/services/julia/juliaRuntimeService', () => ({
  JuliaRuntimeService: {
    install: runtime.install,
  },
}));

vi.mock('@/features/core/ui/UIStore', () => ({
  uiStore: ui,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

import { installJuliaPlugin } from './installJuliaPlugin';

const translate = ((key: string) => key) as unknown as TFunction;

describe('installJuliaPlugin', () => {
  beforeEach(() => {
    localStorage.clear();
    usePluginStore.setState({ installedPluginIds: [] });
    vi.clearAllMocks();
    ui.confirm.mockResolvedValue(true);
  });

  it('installs the Julia slot only after the managed runtime is ready', async () => {
    runtime.install.mockResolvedValue({
      state: 'ready',
      version: '1.12.0',
      installDir: null,
    });

    await expect(installJuliaPlugin(translate)).resolves.toBe(true);

    expect(usePluginStore.getState().installedPluginIds).toEqual(['julia']);
    expect(ui.startProgress).toHaveBeenCalledOnce();
    expect(ui.finishProgress).toHaveBeenCalledOnce();
    expect(ui.alert).not.toHaveBeenCalled();
  });

  it('keeps the Julia slot absent when runtime installation is invalid', async () => {
    runtime.install.mockResolvedValue({
      state: 'invalid',
      version: null,
      installDir: null,
    });
    ui.alert.mockResolvedValue(undefined);

    await expect(installJuliaPlugin(translate)).resolves.toBe(false);

    expect(usePluginStore.getState().installedPluginIds).toEqual([]);
    expect(ui.alert).toHaveBeenCalledOnce();
  });
});
