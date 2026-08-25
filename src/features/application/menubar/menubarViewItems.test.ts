import { describe, expect, it, vi } from 'vitest';
import {
  buildViewMenuItems,
  type MenubarViewMenuActions,
  type MenubarViewState,
} from './menubarViewItems';

const t = ((key: string) => key) as never;

function actions(): MenubarViewMenuActions {
  return {
    toggleActivityGroup: vi.fn(),
    toggleInspect: vi.fn(),
    toggleLogs: vi.fn(),
    toggleOutput: vi.fn(),
    resetLayout: vi.fn(),
  };
}

function state(overrides: Partial<MenubarViewState> = {}): MenubarViewState {
  return {
    activityGroupOpen: true,
    inspectOpen: false,
    inspectContextValid: false,
    logsOpen: true,
    outputOpen: true,
    bottomCollapsed: false,
    ...overrides,
  };
}

describe('buildViewMenuItems', () => {
  it('emits the four toggleable root views and Reset Layout with live checked state', () => {
    const items = buildViewMenuItems(t, state({ bottomCollapsed: true }), actions());

    expect(items.map((item) => item.label)).toEqual([
      'panel.primarySideBar',
      'panel.inspect',
      'panel.logs',
      'panel.output',
      '-',
      'menubar.resetLayout',
    ]);
    expect(items[0]).toMatchObject({ type: 'checkbox', checked: true });
    expect(items[3]).toMatchObject({ type: 'checkbox', checked: true });
  });

  it('disables Inspect only when both panel and context are absent', () => {
    const callbacks = actions();
    const unavailable = buildViewMenuItems(t, state(), callbacks);
    expect(unavailable[1]?.onClick).toBeUndefined();

    const available = buildViewMenuItems(t, state({
      inspectContextValid: true,
    }), callbacks);
    expect(available.find((item) => item.label === 'panel.inspect')?.onClick)
      .toBe(callbacks.toggleInspect);
  });
});
