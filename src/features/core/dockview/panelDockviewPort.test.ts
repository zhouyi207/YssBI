import { describe, expect, it, vi } from 'vitest';
import type {
  DockviewApi,
  DockviewGroupPanelApi,
  IDockviewPanel,
} from 'dockview-react';

import { createPanelDockviewPort } from './panelDockviewPort';
import type { DockviewLayout } from './types';

function disposable() {
  return { dispose: vi.fn() };
}

function createEvent<T>() {
  const listeners = new Set<(event: T) => void>();
  return {
    event(listener: (event: T) => void) {
      listeners.add(listener);
      return { dispose: () => listeners.delete(listener) };
    },
    fire(event: T) {
      listeners.forEach((listener) => listener(event));
    },
  };
}

describe('panelDockviewPort', () => {
  it('preserves the expanded size across repeated collapse and tab activation', async () => {
    let collapsed = false;
    const collapsedChange = createEvent<{ isCollapsed: boolean }>();
    const activePanels: string[] = [];
    const panels = new Map<string, IDockviewPanel>([
      ['logs', { api: { setActive: () => activePanels.push('logs') } } as unknown as IDockviewPanel],
      ['output', { api: { setActive: () => activePanels.push('output') } } as unknown as IDockviewPanel],
    ]);
    const group = {
      id: 'workbench-panel-bottom',
      isCollapsed: () => collapsed,
      collapse: () => {
        collapsed = true;
        collapsedChange.fire({ isCollapsed: true });
      },
      expand: () => {
        collapsed = false;
        collapsedChange.fire({ isCollapsed: false });
      },
      onDidCollapsedChange: collapsedChange.event,
    } as unknown as DockviewGroupPanelApi;
    const layout = {
      grid: { root: { type: 'branch', data: [] }, height: 800, width: 1200, orientation: 'HORIZONTAL' },
      panels: {},
      edgeGroups: { bottom: { size: 320, visible: true } },
    } as DockviewLayout;
    const api = {
      getEdgeGroup: (position: string) => position === 'bottom' ? group : undefined,
      getPanel: (id: string) => panels.get(id),
      toJSON: () => layout,
      onDidLayoutChange: () => disposable(),
      onDidActivePanelChange: () => disposable(),
      onDidLayoutFromJSON: () => disposable(),
    } as unknown as DockviewApi;
    const port = createPanelDockviewPort();
    const unboundSnapshot = port.getSnapshot();
    port.bind(api);

    expect(unboundSnapshot).toMatchObject({ ready: false, collapsed: undefined });
    expect(port.getSnapshot()).toMatchObject({ ready: true, collapsed: false });

    for (let iteration = 0; iteration < 5; iteration += 1) {
      await port.setCollapsed(true);
      expect(port.getSnapshot().collapsed).toBe(true);
      await port.activate(iteration % 2 === 0 ? 'logs' : 'output');
      await port.setCollapsed(false);
      expect(port.getSnapshot().collapsed).toBe(false);
    }

    expect(port.getSnapshot().collapsed).toBe(false);
    expect((await port.serialize()).edgeGroups?.bottom?.size).toBe(320);
    expect(activePanels).toEqual(['logs', 'output', 'logs', 'output', 'logs']);
  });
});
