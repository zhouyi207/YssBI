import type { SerializedDockview } from 'dockview-react';
import { describe, expect, it } from 'vitest';

import {
  createDefaultLogsDockviewLayout,
  DEFAULT_LOGS_DOCKVIEW_LAYOUT,
  LOGS_DOCKVIEW_COMPONENT_ID,
} from './logsDockviewLayout';
import { logDomainPanelId } from '@/features/core/log/logDomains';
import {
  createPersistedWorkbenchLayout,
  parsePersistedWorkbenchLayout,
  prepareRootLayoutForPersistence,
  scrubProjectScopedRootLayout,
  workbenchLayoutStorageKey,
} from './workbenchLayoutPersistence';

type TestPanel = {
  readonly component: string;
  readonly metadata: Record<string, unknown>;
  readonly title?: string;
};

type MutableGroup = {
  views: string[];
  activeView?: string;
};

type GridWithMaximizedNode = SerializedDockview['grid'] & {
  maximizedNode?: unknown;
};

const ACTIVITY_PANELS: Readonly<Record<string, TestPanel>> = {
  project: {
    component: 'Project',
    metadata: { role: 'view', viewId: 'project' },
  },
  nodes: {
    component: 'Nodes',
    metadata: { role: 'view', viewId: 'nodes' },
  },
  data: {
    component: 'Data',
    metadata: { role: 'view', viewId: 'data' },
  },
  commands: {
    component: 'Commands',
    metadata: { role: 'view', viewId: 'commands' },
  },
};

function gridWithMaximizedNode(layout: SerializedDockview): GridWithMaximizedNode {
  return layout.grid as GridWithMaximizedNode;
}

function getOnlyGridGroup(layout: SerializedDockview): MutableGroup {
  const root = layout.grid.root;
  if (root.type !== 'branch' || !Array.isArray(root.data) || root.data.length !== 1) {
    throw new Error('fixture must use a top-level branch with one group');
  }
  const child = root.data[0];
  if (child.type !== 'leaf') throw new Error('fixture branch child must be a group leaf');
  return child.data as MutableGroup;
}

function rootLayout(
  panels: Readonly<Record<string, TestPanel>> = {
    editor: {
      component: 'GraphEditor',
      metadata: {
        role: 'editor',
        resourceRef: 'events/Main.yssbi-event',
        resourceKind: 'event',
      },
    },
  },
): SerializedDockview {
  const allPanels = { ...panels, ...ACTIVITY_PANELS };
  const panelIds = Object.keys(allPanels);
  const activityPanelIds = Object.entries(allPanels)
    .filter(([, panel]) => panel.metadata.role === 'view'
      && ['project', 'nodes', 'data', 'commands'].includes(String(panel.metadata.viewId)))
    .map(([id]) => id);
  const gridPanelIds = panelIds.filter((id) => !activityPanelIds.includes(id));
  return {
    grid: {
      root: {
        type: 'branch',
        data: [{
          type: 'leaf',
          data: {
            id: 'grid-main',
            views: gridPanelIds,
            ...(gridPanelIds[0] === undefined ? {} : { activeView: gridPanelIds[0] }),
          },
        }],
      },
      height: 800,
      width: 1200,
      orientation: 'HORIZONTAL',
    },
    panels: Object.fromEntries(Object.entries(allPanels).map(([id, panel]) => [
      id,
      {
        id,
        contentComponent: panel.component,
        ...(panel.title === undefined ? {} : { title: panel.title }),
        params: { metadata: panel.metadata },
      },
    ])),
    activeGroup: 'grid-main',
    floatingGroups: [],
    popoutGroups: [],
    edgeGroups: {
      left: {
        size: 292,
        visible: true,
        collapsed: false,
        group: {
          id: 'workbench-edge-left',
          views: activityPanelIds,
          activeView: activityPanelIds[0],
          headerPosition: 'left',
        },
      },
    },
  } as unknown as SerializedDockview;
}

function payload(root: unknown, logs: unknown = createDefaultLogsDockviewLayout()): unknown {
  return {
    root,
    nested: { logs },
  };
}

function parsedRootStatus(layout: SerializedDockview): 'valid' | 'invalid' {
  const parsed = parsePersistedWorkbenchLayout(payload(layout));
  if (!parsed) throw new Error('expected a well-formed persistence envelope');
  return parsed.root.status;
}

function duplicateLogsDomainLayout(): SerializedDockview {
  const layout = createDefaultLogsDockviewLayout();
  const group = getOnlyGridGroup(layout);

  const duplicateId = 'logs-domain:all-copy';
  layout.panels[duplicateId] = {
    id: duplicateId,
    contentComponent: LOGS_DOCKVIEW_COMPONENT_ID,
    title: 'All copy',
    params: { domain: 'all' },
  };
  group.views.push(duplicateId);
  return layout;
}

function missingLogsDomainLayout(): SerializedDockview {
  const layout = createDefaultLogsDockviewLayout();
  const group = getOnlyGridGroup(layout);
  const missingPanelId = logDomainPanelId('ui');
  delete layout.panels[missingPanelId];
  group.views = group.views.filter((panelId) => panelId !== missingPanelId);
  return layout;
}

describe('workbench layout persistence', () => {
  it('uses the current semantic key and exact unversioned envelope', () => {
    const root = rootLayout();
    const logs = createDefaultLogsDockviewLayout();

    expect(workbenchLayoutStorageKey('main')).toBe('yssbi-workbench-layout:main');
    expect(workbenchLayoutStorageKey('')).toBe('yssbi-workbench-layout:main');
    expect(createPersistedWorkbenchLayout(root, logs)).toEqual({
      root,
      nested: { logs },
    });
    expect(createPersistedWorkbenchLayout(root, logs)).not.toHaveProperty('version');
    expect(parsePersistedWorkbenchLayout({
      ...payload(root, logs) as Record<string, unknown>,
      version: 1,
    })).toBeNull();
    expect(parsePersistedWorkbenchLayout({ root, nested: { logs } })).not.toBeNull();
  });

  it('defines one deterministic default Logs group with all seven domains', () => {
    const first = createDefaultLogsDockviewLayout();
    const second = createDefaultLogsDockviewLayout();

    expect(first).toEqual(DEFAULT_LOGS_DOCKVIEW_LAYOUT);
    expect(second).toEqual(first);
    expect(second).not.toBe(first);
    expect(Object.values(first.panels).map((panel) => panel.params?.domain)).toEqual([
      'all',
      'application',
      'execution',
      'system',
      'graph',
      'data',
      'ui',
    ]);
    expect(Object.values(first.panels).every(
      (panel) => panel.contentComponent === LOGS_DOCKVIEW_COMPONENT_ID,
    )).toBe(true);
    expect(first.grid.root.type).toBe('branch');
    expect(getOnlyGridGroup(first).activeView).toBe('logs-domain:all');
  });

  it('validates root and nested Logs snapshots independently', () => {
    const validRoot = rootLayout();
    const validLogs = createDefaultLogsDockviewLayout();
    const invalidLogs = duplicateLogsDomainLayout();
    const missingLogs = missingLogsDomainLayout();

    const rootPreserved = parsePersistedWorkbenchLayout(payload(validRoot, invalidLogs));
    expect(rootPreserved).not.toBeNull();
    expect(rootPreserved?.root).toEqual({ status: 'valid', value: validRoot });
    expect(rootPreserved?.logs).toEqual({ status: 'invalid' });

    const missingLogsResult = parsePersistedWorkbenchLayout(payload(validRoot, missingLogs));
    expect(missingLogsResult?.root.status).toBe('valid');
    expect(missingLogsResult?.logs.status).toBe('invalid');

    const invalidRoot = structuredClone(validRoot);
    invalidRoot.panels.editor.contentComponent = 'WorksheetEditor';
    const logsPreserved = parsePersistedWorkbenchLayout(payload(invalidRoot, validLogs));
    expect(logsPreserved).not.toBeNull();
    expect(logsPreserved?.root).toEqual({ status: 'invalid' });
    expect(logsPreserved?.logs).toEqual({ status: 'valid', value: validLogs });
  });

  it('rejects non-canonical root metadata, singleton conflicts, and transient panels', () => {
    const unknownMetadata = rootLayout();
    const metadata = unknownMetadata.panels.editor.params?.metadata as Record<string, unknown>;
    metadata.legacyId = 'editor';

    const duplicateSingleton = rootLayout({
      projectOne: {
        component: 'Project',
        metadata: { role: 'view', viewId: 'project' },
      },
      projectTwo: {
        component: 'Project',
        metadata: { role: 'view', viewId: 'project' },
      },
    });
    const details = rootLayout({
      details: {
        component: 'Details',
        metadata: { role: 'view', viewId: 'details' },
      },
    });
    const result = rootLayout({
      result: {
        component: 'Result',
        metadata: {
          role: 'result',
          resultKey: 'summary',
          resultId: '42',
          title: 'Summary',
          presentation: { kind: 'inspector' },
          source: null,
        },
      },
    });

    expect([
      unknownMetadata,
      duplicateSingleton,
      details,
      result,
    ].map(parsedRootStatus)).toEqual([
      'invalid',
      'invalid',
      'invalid',
      'invalid',
    ]);
  });

  it('rejects floating, popout, dangling, duplicate, and inconsistent root references', () => {
    const floating = rootLayout();
    floating.floatingGroups = [{ position: { left: 0, top: 0, width: 100, height: 100 } }] as never;

    const popout = rootLayout();
    popout.popoutGroups = [{ position: null }] as never;

    const topLevelLeaf = rootLayout();
    const topLevelBranch = topLevelLeaf.grid.root;
    if (topLevelBranch.type !== 'branch' || !Array.isArray(topLevelBranch.data)) {
      throw new Error('expected branch fixture');
    }
    topLevelLeaf.grid.root = structuredClone(topLevelBranch.data[0]);

    const danglingPanel = rootLayout();
    getOnlyGridGroup(danglingPanel).views.push('missing-panel');

    const invalidActiveView = rootLayout();
    getOnlyGridGroup(invalidActiveView).activeView = 'missing-panel';

    const missingActiveView = rootLayout();
    delete getOnlyGridGroup(missingActiveView).activeView;

    const emptyActiveView = rootLayout();
    getOnlyGridGroup(emptyActiveView).activeView = '';

    const invalidActiveGroup = rootLayout();
    invalidActiveGroup.activeGroup = 'missing-group';

    const duplicateGroup = rootLayout();
    duplicateGroup.edgeGroups = {
      right: {
        size: 300,
        visible: true,
        group: {
          id: 'grid-main',
          views: [],
          activeView: '',
        },
      },
    };

    expect([
      floating,
      popout,
      topLevelLeaf,
      danglingPanel,
      invalidActiveView,
      missingActiveView,
      emptyActiveView,
      invalidActiveGroup,
      duplicateGroup,
    ].map(parsedRootStatus)).toEqual([
      'invalid',
      'invalid',
      'invalid',
      'invalid',
      'invalid',
      'invalid',
      'invalid',
      'invalid',
      'invalid',
    ]);
  });

  it('rejects unknown Logs domains and invalid components or topology', () => {
    const unknownDomain = createDefaultLogsDockviewLayout();
    unknownDomain.panels['logs-domain:all'].params = { domain: 'network' };

    const wrongComponent = createDefaultLogsDockviewLayout();
    wrongComponent.panels['logs-domain:all'].contentComponent = 'Logs';

    const topLevelLeaf = createDefaultLogsDockviewLayout();
    const topLevelBranch = topLevelLeaf.grid.root;
    if (topLevelBranch.type !== 'branch' || !Array.isArray(topLevelBranch.data)) {
      throw new Error('expected branch fixture');
    }
    topLevelLeaf.grid.root = structuredClone(topLevelBranch.data[0]);

    const danglingReference = createDefaultLogsDockviewLayout();
    getOnlyGridGroup(danglingReference).views.push('missing-domain-panel');

    const missingActiveView = createDefaultLogsDockviewLayout();
    delete getOnlyGridGroup(missingActiveView).activeView;

    const emptyActiveView = createDefaultLogsDockviewLayout();
    getOnlyGridGroup(emptyActiveView).activeView = '';

    const invalidActiveGroup = createDefaultLogsDockviewLayout();
    invalidActiveGroup.activeGroup = 'missing-group';

    for (const logs of [
      unknownDomain,
      duplicateLogsDomainLayout(),
      wrongComponent,
      topLevelLeaf,
      danglingReference,
      missingActiveView,
      emptyActiveView,
      invalidActiveGroup,
    ]) {
      const parsed = parsePersistedWorkbenchLayout(payload(rootLayout(), logs));
      expect(parsed?.root.status).toBe('valid');
      expect(parsed?.logs.status).toBe('invalid');
    }
  });

  it('validates maximized descriptors and requires their paths to resolve to leaves', () => {
    const validRoot = rootLayout();
    gridWithMaximizedNode(validRoot).maximizedNode = { location: [0] };
    const validLogs = createDefaultLogsDockviewLayout();
    gridWithMaximizedNode(validLogs).maximizedNode = { location: [0] };

    const parsed = parsePersistedWorkbenchLayout(payload(validRoot, validLogs));
    expect(parsed?.root).toEqual({ status: 'valid', value: validRoot });
    expect(parsed?.logs).toEqual({ status: 'valid', value: validLogs });
    expect(gridWithMaximizedNode(
      prepareRootLayoutForPersistence(validRoot),
    ).maximizedNode).toEqual({ location: [0] });

    const invalidDescriptors: unknown[] = [
      {},
      { location: [] },
      { location: [-1] },
      { location: [0.5] },
      { location: [Number.POSITIVE_INFINITY] },
      { location: [1] },
      { location: [0, 0] },
      { location: [0], extra: true },
    ];
    for (const descriptor of invalidDescriptors) {
      const invalidRoot = rootLayout();
      gridWithMaximizedNode(invalidRoot).maximizedNode = descriptor;
      expect(parsedRootStatus(invalidRoot)).toBe('invalid');

      const invalidLogs = createDefaultLogsDockviewLayout();
      gridWithMaximizedNode(invalidLogs).maximizedNode = descriptor;
      expect(parsePersistedWorkbenchLayout(payload(rootLayout(), invalidLogs))?.logs.status)
        .toBe('invalid');
    }
  });

  it('strips transients and clears maximization when pruning a transient-only leaf', () => {
    const layout = rootLayout({
      editor: {
        component: 'GraphEditor',
        metadata: {
          role: 'editor',
          resourceRef: 'events/Main.yssbi-event',
          resourceKind: 'event',
        },
      },
      details: {
        component: 'Details',
        metadata: { role: 'view', viewId: 'details' },
      },
      inspect: {
        component: 'Inspect',
        metadata: { role: 'view', viewId: 'inspect' },
      },
      result: {
        component: 'Result',
        metadata: {
          role: 'result',
          resultKey: 'summary',
          resultId: '42',
          title: 'Summary',
          presentation: { kind: 'inspector' },
          source: null,
        },
      },
      logs: {
        component: 'Logs',
        metadata: { role: 'view', viewId: 'logs' },
      },
    });
    layout.grid.root = {
      type: 'branch',
      data: [
        {
          type: 'leaf',
          data: {
            id: 'grid-main',
            views: ['editor', 'details'],
            activeView: 'details',
          },
        },
        {
          type: 'leaf',
          data: {
            id: 'transient-active',
            views: ['inspect'],
            activeView: 'inspect',
          },
        },
        {
          type: 'leaf',
          data: {
            id: 'empty-grid',
            views: [],
            activeView: '',
          },
        },
      ],
    };
    layout.edgeGroups = {
      left: {
        size: 292,
        visible: true,
        collapsed: false,
        group: {
          id: 'workbench-edge-left',
          views: ['project', 'nodes', 'data', 'commands'],
          activeView: 'project',
          headerPosition: 'left',
        },
      },
      right: {
        size: 320,
        visible: true,
        group: {
          id: 'result-edge',
          views: ['result'],
          activeView: 'result',
        },
      },
      bottom: {
        size: 200,
        visible: true,
        group: {
          id: 'logs-edge',
          views: ['logs'],
          activeView: 'logs',
        },
      },
    };
    gridWithMaximizedNode(layout).maximizedNode = { location: [1] };
    layout.activeGroup = 'transient-active';
    const original = structuredClone(layout);

    const persisted = prepareRootLayoutForPersistence(layout);

    expect(layout).toEqual(original);
    expect(persisted).not.toBe(layout);
    expect(Object.keys(persisted.panels)).toEqual([
      'editor',
      'logs',
      'project',
      'nodes',
      'data',
      'commands',
    ]);
    expect(persisted.grid.root).toEqual({
      type: 'branch',
      data: [{
        type: 'leaf',
        data: {
          id: 'grid-main',
          views: ['editor'],
          activeView: 'editor',
        },
      }],
    });
    expect(gridWithMaximizedNode(persisted).maximizedNode).toBeUndefined();
    expect(persisted.edgeGroups?.right).toBeUndefined();
    expect(persisted.edgeGroups?.left?.group).toEqual({
      id: 'workbench-edge-left',
      views: ['project', 'nodes', 'data', 'commands'],
      activeView: 'project',
      headerPosition: 'left',
    });
    expect(persisted.edgeGroups?.bottom?.group).toEqual({
      id: 'logs-edge',
      views: ['logs'],
      activeView: 'logs',
    });
    expect(persisted.activeGroup).toBe('grid-main');
    expect(parsedRootStatus(persisted)).toBe('valid');
  });

  it('scrubs project-scoped panels while preserving tool panel topology', () => {
    const layout = rootLayout({
      editor: {
        component: 'GraphEditor',
        metadata: {
          role: 'editor',
          resourceRef: 'events/Main.yssbi-event',
          resourceKind: 'event',
        },
      },
      details: {
        component: 'Details',
        metadata: { role: 'view', viewId: 'details' },
      },
      inspect: {
        component: 'Inspect',
        metadata: { role: 'view', viewId: 'inspect' },
      },
      result: {
        component: 'Result',
        metadata: {
          role: 'result',
          resultKey: 'summary',
          resultId: '42',
          title: 'Summary',
          presentation: { kind: 'inspector' },
          source: null,
        },
      },
      logs: {
        component: 'Logs',
        metadata: { role: 'view', viewId: 'logs' },
      },
      output: {
        component: 'Output',
        metadata: { role: 'view', viewId: 'output' },
      },
    });
    layout.grid.root = {
      type: 'branch',
      data: [{
        type: 'leaf',
        data: {
          id: 'project-grid',
          views: ['editor', 'inspect'],
          activeView: 'editor',
          tabGroups: [{
            id: 'project-tabs',
            collapsed: false,
            panelIds: ['editor', 'inspect'],
          }],
        },
      }],
    };
    layout.edgeGroups = {
      left: {
        size: 292,
        visible: true,
        collapsed: false,
        group: {
          id: 'workbench-edge-left',
          views: ['project', 'nodes', 'data', 'commands'],
          activeView: 'project',
          headerPosition: 'left',
        },
      },
      bottom: {
        size: 200,
        visible: true,
        collapsed: true,
        group: {
          id: 'tools-edge',
          views: ['logs', 'result', 'output'],
          activeView: 'result',
          tabGroups: [{
            id: 'tools-tabs',
            collapsed: false,
            panelIds: ['logs', 'result', 'output'],
          }],
        },
      },
    };
    gridWithMaximizedNode(layout).maximizedNode = { location: [0] };
    layout.activeGroup = 'project-grid';
    const original = structuredClone(layout);

    const scrubbed = scrubProjectScopedRootLayout(layout);

    expect(layout).toEqual(original);
    expect(Object.keys(scrubbed.panels)).toEqual([
      'logs',
      'output',
      'project',
      'nodes',
      'data',
      'commands',
    ]);
    expect(scrubbed.grid.root).toEqual({ type: 'branch', data: [] });
    expect(gridWithMaximizedNode(scrubbed).maximizedNode).toBeUndefined();
    expect(scrubbed.edgeGroups).toEqual({
      left: {
        size: 292,
        visible: true,
        collapsed: false,
        group: {
          id: 'workbench-edge-left',
          views: ['project', 'nodes', 'data', 'commands'],
          activeView: 'project',
          headerPosition: 'left',
        },
      },
      bottom: {
        size: 200,
        visible: true,
        collapsed: true,
        group: {
          id: 'tools-edge',
          views: ['logs', 'output'],
          activeView: 'logs',
          tabGroups: [{
            id: 'tools-tabs',
            collapsed: false,
            panelIds: ['logs', 'output'],
          }],
        },
      },
    });
    expect(scrubbed.activeGroup).toBe('tools-edge');
    expect(parsedRootStatus(scrubbed)).toBe('valid');
  });

  it('preserves nested branch depth and sizes when pruning a transient sibling', () => {
    const layout = rootLayout({
      editor: {
        component: 'GraphEditor',
        metadata: {
          role: 'editor',
          resourceRef: 'events/Main.yssbi-event',
          resourceKind: 'event',
        },
      },
      details: {
        component: 'Details',
        metadata: { role: 'view', viewId: 'details' },
      },
    });
    layout.grid.root = {
      type: 'branch',
      data: [{
        type: 'branch',
        size: 720,
        visible: true,
        data: [
          {
            type: 'leaf',
            size: 480,
            visible: true,
            data: {
              id: 'grid-main',
              views: ['editor'],
              activeView: 'editor',
            },
          },
          {
            type: 'leaf',
            size: 240,
            visible: false,
            data: {
              id: 'transient-details',
              views: ['details'],
              activeView: 'details',
            },
          },
        ],
      }],
    };

    const persisted = prepareRootLayoutForPersistence(layout);

    expect(persisted.grid.root).toEqual({
      type: 'branch',
      data: [{
        type: 'branch',
        size: 720,
        visible: true,
        data: [{
          type: 'leaf',
          size: 480,
          visible: true,
          data: {
            id: 'grid-main',
            views: ['editor'],
            activeView: 'editor',
          },
        }],
      }],
    });
    expect(parsedRootStatus(persisted)).toBe('valid');
  });
});
