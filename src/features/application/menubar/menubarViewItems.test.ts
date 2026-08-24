import { describe, expect, it, vi } from 'vitest';
import {
  buildViewMenuItems,
  type MenubarViewMenuActions,
  type MenubarViewState,
} from './menubarViewItems';

const t = ((key: string) => key) as never;

function actions(): MenubarViewMenuActions {
  return {
    toggleResources: vi.fn(),
    toggleDetails: vi.fn(),
    toggleInspect: vi.fn(),
    toggleLogs: vi.fn(),
    toggleOutput: vi.fn(),
    resetLayout: vi.fn(),
  };
}

function state(overrides: Partial<MenubarViewState> = {}): MenubarViewState {
  return {
    resourcesOpen: true,
    detailsOpen: false,
    detailsContextValid: false,
    inspectOpen: false,
    inspectContextValid: false,
    logsOpen: true,
    outputOpen: true,
    bottomCollapsed: false,
    ...overrides,
  };
}

describe('buildViewMenuItems', () => {
  it('emits only the five root views and Reset Layout with live checked state', () => {
    const items = buildViewMenuItems(t, state({ bottomCollapsed: true }), actions());

    expect(items.map((item) => item.label)).toEqual([
      'panel.resources',
      'panel.details',
      'panel.inspect',
      'panel.logs',
      'panel.output',
      '-',
      'menubar.resetLayout',
    ]);
    expect(items[0]).toMatchObject({ type: 'checkbox', checked: true });
    expect(items[3]).toMatchObject({ type: 'checkbox', checked: true });
    expect(items[4]).toMatchObject({ type: 'checkbox', checked: true });
  });

  it('disables Details and Inspect only when both panel and context are absent', () => {
    const callbacks = actions();
    const unavailable = buildViewMenuItems(t, state(), callbacks);
    expect(unavailable[1]?.onClick).toBeUndefined();
    expect(unavailable[2]?.onClick).toBeUndefined();

    const available = buildViewMenuItems(t, state({
      detailsOpen: true,
      inspectContextValid: true,
    }), callbacks);
    expect(available[1]?.onClick).toBe(callbacks.toggleDetails);
    expect(available[2]?.onClick).toBe(callbacks.toggleInspect);
  });
});
