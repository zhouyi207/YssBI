// @vitest-environment happy-dom

import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  DEFAULT_APPEARANCE,
  DEFAULT_EDITOR,
  DEFAULT_PROJECT,
  DEFAULT_THEME,
} from '@/app/appConfig/default';
import { useSettingsStore } from './settingsStore';

vi.mock('@tauri-apps/api/event', () => ({
  emit: vi.fn(async () => undefined),
  listen: vi.fn(async () => () => undefined),
}));

vi.mock('@/utils/appLogger', () => ({
  logger: {
    app: {
      warn: vi.fn(),
      error: vi.fn(),
    },
  },
}));

const SETTINGS_STORAGE_KEY = 'yssbi-client-settings';

const EDITOR_KEYS = [
  'alwaysShowEditorActions',
  'autoSave',
  'closeEmptyGroups',
  'fontSize',
  'openSideBySideDirection',
  'showGrid',
  'snapToGrid',
  'splitOnDragAndDrop',
  'splitSizing',
];

const APPEARANCE_KEYS = [
  'colorTheme',
  'language',
  'lastDarkColorTheme',
  'lastLightColorTheme',
  'smoothScroll',
  'titleBarStyle',
];

describe('settingsStore appearance persistence', () => {
  beforeEach(() => {
    localStorage.clear();
    useSettingsStore.setState({
      theme: DEFAULT_THEME,
      editor: DEFAULT_EDITOR,
      appearance: DEFAULT_APPEARANCE,
      project: DEFAULT_PROJECT,
      isLoading: true,
    });
  });

  it('projects known editor fields and removes stored drag-to-window options', async () => {
    localStorage.setItem(SETTINGS_STORAGE_KEY, JSON.stringify({
      editor: {
        showGrid: false,
        autoSave: false,
        snapToGrid: false,
        fontSize: 14,
        openSideBySideDirection: 'down',
        splitOnDragAndDrop: false,
        alwaysShowEditorActions: true,
        closeEmptyGroups: false,
        splitSizing: 'distribute',
        dragToOpenWindow: true,
        futureEditorField: 'discard me',
      },
    }));

    await useSettingsStore.getState().load();

    const loaded = useSettingsStore.getState().editor as unknown as Record<string, unknown>;
    expect(Object.keys(loaded).sort()).toEqual(EDITOR_KEYS);
    expect(loaded).not.toHaveProperty('dragToOpenWindow');
    expect(loaded).not.toHaveProperty('futureEditorField');

    await useSettingsStore.getState().save();

    const saved = JSON.parse(localStorage.getItem(SETTINGS_STORAGE_KEY) ?? '{}') as {
      editor?: Record<string, unknown>;
    };
    expect(Object.keys(saved.editor ?? {}).sort()).toEqual(EDITOR_KEYS);
    expect(saved.editor).not.toHaveProperty('dragToOpenWindow');
  });

  it('projects known appearance fields and removes stored panelPosition on save', async () => {
    localStorage.setItem(SETTINGS_STORAGE_KEY, JSON.stringify({
      appearance: {
        colorTheme: 'OLED Black',
        language: 'en-US',
        activityBarPosition: 'Right',
        smoothScroll: false,
        titleBarStyle: 'native',
        panelPosition: 'Left',
        futureAppearanceField: 'discard me',
      },
    }));

    await useSettingsStore.getState().load();

    const loaded = useSettingsStore.getState().appearance as unknown as Record<string, unknown>;
    expect(Object.keys(loaded).sort()).toEqual(APPEARANCE_KEYS);
    expect(loaded).not.toHaveProperty('panelPosition');
    expect(loaded).not.toHaveProperty('futureAppearanceField');
    expect(loaded.lastLightColorTheme).toBe(DEFAULT_APPEARANCE.lastLightColorTheme);
    expect(loaded.lastDarkColorTheme).toBe(DEFAULT_APPEARANCE.lastDarkColorTheme);

    await useSettingsStore.getState().save();

    const saved = JSON.parse(localStorage.getItem(SETTINGS_STORAGE_KEY) ?? '{}') as {
      appearance?: Record<string, unknown>;
    };
    expect(Object.keys(saved.appearance ?? {}).sort()).toEqual(APPEARANCE_KEYS);
    expect(saved.appearance).not.toHaveProperty('panelPosition');
  });
});
