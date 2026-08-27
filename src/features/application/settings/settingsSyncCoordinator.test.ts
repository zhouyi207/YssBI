// @vitest-environment happy-dom

import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  DEFAULT_APPEARANCE,
  DEFAULT_EDITOR,
  DEFAULT_PROJECT,
  DEFAULT_THEME,
} from '@/app/appConfig/default';
import type { AppSettings } from '@/shared/types/settings';
import type { PlatformOutcome } from '@/services/platform/platformTypes';
import { useSettingsStore } from '@/features/core/settings/settingsStore';
import { SettingsSyncCoordinator } from './settingsSyncCoordinator';

const mocks = vi.hoisted(() => ({
  publishSettingsChanged: vi.fn(),
  subscribeSettingsChanged: vi.fn(),
}));

vi.mock('@/services/platform/settingsEvents', () => mocks);

describe('SettingsSyncCoordinator', () => {
  let coordinator: SettingsSyncCoordinator;
  let receiveSettings: ((outcome: PlatformOutcome<AppSettings>) => void) | undefined;

  beforeEach(() => {
    vi.clearAllMocks();
    receiveSettings = undefined;
    useSettingsStore.setState({
      theme: DEFAULT_THEME,
      editor: DEFAULT_EDITOR,
      appearance: DEFAULT_APPEARANCE,
      project: DEFAULT_PROJECT,
      isLoading: true,
    });
    mocks.publishSettingsChanged.mockResolvedValue({ ok: true, value: undefined });
    mocks.subscribeSettingsChanged.mockImplementation(async (
      listener: (outcome: PlatformOutcome<AppSettings>) => void,
    ) => {
      receiveSettings = listener;
      return { ok: true, value: vi.fn() };
    });
    coordinator = new SettingsSyncCoordinator();
  });

  it('publishes local settings and applies remote settings without echoing them', async () => {
    await coordinator.start();

    await useSettingsStore.getState().save();
    expect(mocks.publishSettingsChanged).toHaveBeenCalledWith(expect.objectContaining({
      theme: DEFAULT_THEME,
      editor: DEFAULT_EDITOR,
      appearance: DEFAULT_APPEARANCE,
      project: DEFAULT_PROJECT,
    }));

    const remote: AppSettings = {
      theme: { ...DEFAULT_THEME, accentColor: '#123456' },
      editor: DEFAULT_EDITOR,
      appearance: DEFAULT_APPEARANCE,
      project: DEFAULT_PROJECT,
    };
    receiveSettings?.({ ok: true, value: remote });

    expect(useSettingsStore.getState().theme.accentColor).toBe('#123456');
    expect(mocks.publishSettingsChanged).toHaveBeenCalledOnce();

    coordinator.stop();
  });
});
