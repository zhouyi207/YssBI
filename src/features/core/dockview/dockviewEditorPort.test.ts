import type {
  AddPanelOptions,
  DockviewApi,
  DockviewGroupPanel,
  IDockviewGroupPanel,
  IDockviewPanel,
  SerializedDockview,
} from 'dockview-react';
import { describe, expect, it } from 'vitest';

import { createDockviewEditorPort } from './dockviewEditorPort';

function event<T>() {
  const listeners = new Set<(value: T) => void>();
  return {
    subscribe(listener: (value: T) => void) {
      listeners.add(listener);
      return { dispose: () => listeners.delete(listener) };
    },
    emit(value: T) {
      listeners.forEach((listener) => listener(value));
    },
  };
}

class FakeDockview {
  readonly panels: IDockviewPanel[] = [];
  readonly groups: IDockviewGroupPanel[] = [];
  readonly layoutChange = event<void>();
  readonly activeGroupChange = event<DockviewGroupPanel | undefined>();
  readonly activePanelChange = event<unknown>();
  readonly calls: string[] = [];
  restored: SerializedDockview | undefined;
  closeCalls = 0;
  activePanel: IDockviewPanel | undefined;
  activeGroup: IDockviewGroupPanel | undefined;
  readonly api: DockviewApi;

  constructor() {
    this.addGroup('main');
    const self = this;
    this.api = {
      get panels() { return self.panels; },
      get groups() { return self.groups; },
      get activePanel() { return self.activePanel; },
      get activeGroup() { return self.activeGroup as DockviewGroupPanel | undefined; },
      onDidLayoutChange: this.layoutChange.subscribe,
      onDidActiveGroupChange: this.activeGroupChange.subscribe,
      onDidActivePanelChange: this.activePanelChange.subscribe,
      getPanel: (id: string) => self.panels.find((panel) => panel.id === id),
      getGroup: (id: string) => self.groups.find((group) => group.id === id),
      addPanel: (options: AddPanelOptions<object>) => self.addPanel(options),
      toJSON: () => self.layout(),
      fromJSON: (layout: SerializedDockview) => {
        self.calls.push('restore');
        self.restored = layout;
      },
      clear: () => {
        self.calls.push('reset');
        self.panels.splice(0);
      },
    } as unknown as DockviewApi;
  }

  addGroup(id: string): IDockviewGroupPanel {
    const group = {
      id,
      panels: [] as IDockviewPanel[],
      activePanel: undefined as IDockviewPanel | undefined,
    } as unknown as IDockviewGroupPanel;
    this.groups.push(group);
    if (!this.activeGroup) this.activeGroup = group;
    return group;
  }

  addPanel(options: AddPanelOptions<object>): IDockviewPanel {
    this.calls.push(`open:${options.id}`);
    const referenceGroup = 'position' in options
      && options.position
      && 'referenceGroup' in options.position
      ? options.position.referenceGroup
      : undefined;
    const positionedGroup = typeof referenceGroup === 'string'
      ? this.groups.find(({ id }) => id === referenceGroup)
      : referenceGroup as IDockviewGroupPanel | undefined;
    const group = positionedGroup ?? this.activeGroup ?? this.groups[0];
    const panelState = {
      params: options.params,
      title: options.title,
      group,
    };
    const panel = {
      id: options.id,
      get group() { return panelState.group; },
      get params() { return panelState.params; },
      get title() { return panelState.title; },
      api: {
        component: options.component,
        isActive: false,
        setActive: () => this.setActive(panel),
        close: () => {
          this.closeCalls += 1;
          this.remove(panel);
        },
        moveTo: ({ group: target }: { group?: IDockviewGroupPanel }) => {
          if (!target) return;
          this.removeFromGroup(panel);
          panelState.group = target;
          target.panels.push(panel);
        },
        updateParameters: (params: Record<string, unknown>) => {
          panelState.params = params;
        },
      },
    } as unknown as IDockviewPanel;
    this.panels.push(panel);
    group.panels.push(panel);
    if (!options.inactive) this.setActive(panel);
    return panel;
  }

  private setActive(panel: IDockviewPanel): void {
    this.panels.forEach((item) => {
      (item.api as unknown as { isActive: boolean }).isActive = item === panel;
    });
    this.activePanel = panel;
    this.activeGroup = panel.group;
    (panel.group as unknown as { activePanel?: IDockviewPanel }).activePanel = panel;
  }

  private removeFromGroup(panel: IDockviewPanel): void {
    const index = panel.group.panels.indexOf(panel);
    if (index >= 0) panel.group.panels.splice(index, 1);
  }

  private remove(panel: IDockviewPanel): void {
    this.removeFromGroup(panel);
    const index = this.panels.indexOf(panel);
    if (index >= 0) this.panels.splice(index, 1);
    if (this.activePanel === panel) this.activePanel = undefined;
  }

  layout(): SerializedDockview {
    return {
      grid: {
        root: { type: 'leaf', data: { id: 'main', views: [], activeView: '' } },
        height: 100,
        width: 100,
        orientation: 'HORIZONTAL',
      },
      panels: {},
      floatingGroups: [],
      popoutGroups: [],
    } as unknown as SerializedDockview;
  }
}

const graphTab = {
  resourceRef: 'events/shared.yssbi-event',
  kind: 'event',
  data: {
    layoutTab: {
      id: 'events/shared.yssbi-event',
      type: 'event',
      component: 'GraphEditor',
      pinned: false,
    },
  },
} as const;

describe('DockviewEditorPort', () => {
  it('keeps panel instance identity separate and allows duplicate resources', async () => {
    const fake = new FakeDockview();
    const port = createDockviewEditorPort();
    port.bind(fake.api);

    await port.open({
      panelInstanceId: 'panel-1',
      component: 'GraphEditor',
      tab: graphTab,
      groupId: 'main',
    });
    await port.open({
      panelInstanceId: 'panel-2',
      component: 'GraphEditor',
      tab: graphTab,
      groupId: 'main',
    });

    expect(port.findPanelsByResource(graphTab.resourceRef)
      .map((panel) => panel.panelInstanceId)).toEqual(['panel-1', 'panel-2']);
    expect(port.listPanels().map((panel) => panel.tab)).toEqual([graphTab, graphTab]);

    expect(await port.remapResource(graphTab.resourceRef, 'events/renamed.yssbi-event'))
      .toBe(2);
    expect(port.findPanelsByResource('events/renamed.yssbi-event')).toHaveLength(2);
    expect(port.listPanels()[0]?.tab?.data?.layoutTab).toMatchObject({
      id: 'events/renamed.yssbi-event',
      pinned: false,
    });

    await port.updateTab('panel-1', {
      ...graphTab,
      data: { ...graphTab.data, layoutTab: { ...graphTab.data.layoutTab, pinned: true } },
    });
    expect(port.listPanels()[0]?.tab?.data?.layoutTab).toMatchObject({ pinned: true });
  });

  it('sanitizes floating and popout groups without mutating restore input', async () => {
    const fake = new FakeDockview();
    const port = createDockviewEditorPort();
    const layout = fake.layout();
    port.bind(fake.api);

    await port.restore(layout);

    expect(fake.restored).not.toHaveProperty('floatingGroups');
    expect(fake.restored).not.toHaveProperty('popoutGroups');
    expect(layout).toHaveProperty('floatingGroups');
    expect(layout).toHaveProperty('popoutGroups');
  });

  it('queues commands before bind without exposing a writable topology copy', async () => {
    const port = createDockviewEditorPort();
    const fake = new FakeDockview();
    const initialSnapshot = port.getSnapshot();
    const pendingReset = port.reset();
    const pendingOpen = port.open({
      panelInstanceId: 'pending-panel',
      component: 'GraphEditor',
      tab: graphTab,
    });

    expect(port.listGroups()).toEqual([]);
    expect(port.listPanels()).toEqual([]);
    expect(port.getSnapshot()).toBe(initialSnapshot);
    expect(fake.calls).toEqual([]);

    port.bind(fake.api);
    await Promise.all([pendingReset, pendingOpen]);

    expect(fake.calls).toEqual(['reset', 'open:pending-panel']);
    expect(port.listPanels()).toHaveLength(1);
    expect(port.getSnapshot()).toMatchObject({ ready: true });
    expect(port.getSnapshot().revision).toBeGreaterThan(initialSnapshot.revision);
  });

  it('publishes from Dockview events instead of duplicating command notifications', async () => {
    const fake = new FakeDockview();
    const port = createDockviewEditorPort();
    port.bind(fake.api);
    let notifications = 0;
    const unsubscribe = port.subscribe(() => {
      notifications += 1;
    });

    await port.serialize();
    await port.open({
      panelInstanceId: 'event-owned-notification',
      component: 'GraphEditor',
      tab: graphTab,
    });
    expect(notifications).toBe(0);

    fake.layoutChange.emit();
    expect(notifications).toBe(1);
    unsubscribe();
  });

  it('removes panels only through panel.api.close', async () => {
    const fake = new FakeDockview();
    const port = createDockviewEditorPort();
    port.bind(fake.api);
    await port.open({
      panelInstanceId: 'closable',
      component: 'GraphEditor',
      tab: graphTab,
    });

    expect(await port.remove('closable')).toBe(true);
    expect(fake.closeCalls).toBe(1);
    expect(port.listPanels()).toEqual([]);
  });
});
