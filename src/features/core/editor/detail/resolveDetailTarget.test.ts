import { describe, expect, it } from 'vitest';
import type { LayoutTab } from '@/shared/types/layout/layout';
import type { LogMessage } from '@/shared/types/ui';
import { resolveDetailTarget } from './resolveDetailTarget';

const tabs: LayoutTab[] = [
  { id: 'g1', title: 'Event 1', component: 'GraphEditor', type: 'event' },
  { id: 'ws1', title: 'Sheet 1', component: 'WorksheetEditor', type: 'worksheet' },
];

const logEntry = { level: 'info', message: 'test' } as LogMessage;

function resolve(overrides: Partial<Parameters<typeof resolveDetailTarget>[0]> = {}) {
  return resolveDetailTarget({
    activeTabId: 'g1',
    tabs,
    selectedNodeIds: [],
    sidebarDetailFocus: null,
    selectedLog: null,
    ...overrides,
  });
}

describe('resolveDetailTarget', () => {
  it('returns node when a single node is selected on an active tab', () => {
    expect(
      resolve({
        selectedNodeIds: ['node-1'],
        sidebarDetailFocus: { id: 'var-1', type: 'variable' },
      }),
    ).toEqual({ kind: 'node', id: 'node-1', graphId: 'g1' });
  });

  it('prefers sidebar variable over active event tab', () => {
    expect(
      resolve({
        sidebarDetailFocus: { id: 'var-1', type: 'variable' },
      }),
    ).toEqual({ kind: 'variable', id: 'var-1' });
  });

  it('prefers sidebar data over active tab', () => {
    expect(
      resolve({
        sidebarDetailFocus: { id: 'df-1', type: 'data' },
      }),
    ).toEqual({ kind: 'data', id: 'df-1' });
  });

  it('returns log when no sidebar focus is set', () => {
    expect(
      resolve({
        selectedLog: logEntry,
      }),
    ).toEqual({ kind: 'log' });
  });

  it('derives event detail from active tab when nothing else is focused', () => {
    expect(resolve()).toEqual({ kind: 'event', id: 'g1' });
  });

  it('derives worksheet detail from active tab', () => {
    expect(
      resolve({
        activeTabId: 'ws1',
      }),
    ).toEqual({ kind: 'worksheet', id: 'ws1' });
  });

  it('falls back to sidebar focus after node selection is cleared', () => {
    expect(
      resolve({
        selectedNodeIds: [],
        sidebarDetailFocus: { id: 'var-1', type: 'variable' },
      }),
    ).toEqual({ kind: 'variable', id: 'var-1' });
  });

  it('falls back to tab detail after node selection is cleared without sidebar focus', () => {
    expect(
      resolve({
        selectedNodeIds: [],
      }),
    ).toEqual({ kind: 'event', id: 'g1' });
  });

  it('returns null for settings tab or missing tab', () => {
    expect(
      resolve({
        activeTabId: 'settings',
        tabs: [{ id: 'settings', title: 'Settings', component: 'Settings', type: 'setting' }],
      }),
    ).toBeNull();

    expect(
      resolve({
        activeTabId: null,
        tabs: [],
      }),
    ).toBeNull();
  });

  it('does not return node when active tab is missing', () => {
    expect(
      resolve({
        activeTabId: null,
        selectedNodeIds: ['node-1'],
      }),
    ).toBeNull();
  });
});
