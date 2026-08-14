import { describe, expect, it } from 'vitest';

import {
  createInitialWorkbenchNodes,
  DEFAULT_EDITOR_GROUP_ID,
  EDITOR_AREA_ID,
} from '@/features/core/layout/workbenchLayoutDefaults';
import type { ProjectIndexRow } from '@/services/project/projectService';

import {
  buildAuthoritativeProjectLoadPlan,
  defaultAuthoritativeProjectLoadPlanDependencies,
} from './authoritativeProjectLoadPlan';

const projectInstanceId = '00000000-0000-0000-0000-000000000801';

function emptyIndex(): ProjectIndexRow {
  return {
    projectInstanceId,
    projectName: 'Layout recovery',
    exportTime: '',
    publicationRevision: 1,
    history: { canUndo: false, canRedo: false },
    graphs: [],
    variables: [],
    worksheets: [],
    databases: [],
  };
}

describe('authoritative project load layout normalization', () => {
  it('retains only authoritative worksheet paths and preserves valid active selection', () => {
    const nodes = createInitialWorkbenchNodes();
    const validPath = 'worksheets/Opaque Path With Spaces.yssbi-worksheet';
    const stalePath = 'worksheets/Stale.yssbi-worksheet';
    const index = emptyIndex();
    index.worksheets = [{
      worksheetPath: validPath,
      name: 'Rust supplied label',
      databaseId: 'database-1',
      chartType: 'scatter',
      revision: 7,
    }];

    const plan = buildAuthoritativeProjectLoadPlan(
      { path: null, databases: {}, index },
      {
        databases: {},
        layoutNodes: nodes,
        editorTabs: {
          registry: {
            [validPath]: {
              id: validPath,
              component: 'WorksheetEditor',
              type: 'worksheet',
            },
            [stalePath]: {
              id: stalePath,
              component: 'WorksheetEditor',
              type: 'worksheet',
            },
            settings: { id: 'settings', component: 'GraphEditor', type: 'setting' },
          },
          placements: {
            [DEFAULT_EDITOR_GROUP_ID]: {
              tabIds: ['settings', stalePath, validPath],
              activeTabId: validPath,
              selectedNodeIds: ['node-kept'],
              selectedConnectionIds: [],
              selectedTabIds: [validPath, stalePath],
            },
          },
        },
        recentEditorGroupIds: [DEFAULT_EDITOR_GROUP_ID],
        detailFocus: { kind: 'worksheet', worksheetPath: validPath },
      },
      {
        ...defaultAuthoritativeProjectLoadPlanDependencies,
        validateCoordinatorStart: () => undefined,
      },
    );

    expect(plan.storeState.detailFocus).toEqual({
      kind: 'worksheet',
      worksheetPath: validPath,
    });
    expect(plan.storeState.worksheetIndex).toEqual([{
      worksheetPath: validPath,
      name: 'Rust supplied label',
      databaseId: 'database-1',
      chartType: 'scatter',
      revision: 7,
    }]);
    expect(plan.storeState.layout.tabs.registry).toEqual({
      [validPath]: {
        id: validPath,
        component: 'WorksheetEditor',
        type: 'worksheet',
      },
      settings: { id: 'settings', component: 'GraphEditor', type: 'setting' },
    });
    expect(plan.storeState.layout.tabs.placements[DEFAULT_EDITOR_GROUP_ID]).toEqual({
      tabIds: ['settings', validPath],
      activeTabId: validPath,
      selectedNodeIds: [],
      selectedConnectionIds: [],
      selectedTabIds: [validPath],
    });
  });

  it('recreates the canonical default editor group when runtime split layout removed it', () => {
    const nodes = createInitialWorkbenchNodes();
    delete nodes[DEFAULT_EDITOR_GROUP_ID];
    nodes.split_editor = {
      id: 'split_editor',
      type: 'component',
      parentId: EDITOR_AREA_ID,
      data: { component: 'GraphEditor' },
    };
    nodes[EDITOR_AREA_ID].children = ['split_editor'];

    const plan = buildAuthoritativeProjectLoadPlan(
      { path: null, databases: {}, index: emptyIndex() },
      {
        databases: {},
        layoutNodes: nodes,
        editorTabs: {
          registry: {
            settings: { id: 'settings', component: 'GraphEditor', type: 'setting' },
          },
          placements: {
            split_editor: {
              tabIds: ['settings'],
              activeTabId: 'settings',
              selectedNodeIds: [],
              selectedConnectionIds: [],
              selectedTabIds: ['settings'],
            },
          },
        },
        recentEditorGroupIds: ['split_editor'],
        detailFocus: { kind: 'event', path: 'events/Previous.yssbi-event' },
      },
      {
        ...defaultAuthoritativeProjectLoadPlanDependencies,
        validateCoordinatorStart: () => undefined,
      },
    );

    expect(plan.storeState.detailFocus).toBeNull();
    expect(plan.storeState.layout.nodes[DEFAULT_EDITOR_GROUP_ID]).toMatchObject({
      id: DEFAULT_EDITOR_GROUP_ID,
      type: 'component',
      parentId: EDITOR_AREA_ID,
      data: { component: 'GraphEditor' },
    });
    expect(plan.storeState.layout.nodes.split_editor).toBeUndefined();
    expect(plan.storeState.layout.nodes[EDITOR_AREA_ID].children)
      .toEqual([DEFAULT_EDITOR_GROUP_ID]);
    expect(plan.storeState.layout.activeEditorGroupId).toBe(DEFAULT_EDITOR_GROUP_ID);
    expect(plan.storeState.layout.recentEditorGroupIds).toEqual([DEFAULT_EDITOR_GROUP_ID]);
    expect(plan.storeState.layout.tabs).toEqual({
      registry: {
        settings: { id: 'settings', component: 'GraphEditor', type: 'setting' },
      },
      placements: {
        [DEFAULT_EDITOR_GROUP_ID]: {
          tabIds: ['settings'],
          activeTabId: 'settings',
          selectedNodeIds: [],
          selectedConnectionIds: [],
          selectedTabIds: ['settings'],
        },
      },
    });
  });

  it.each([
    { kind: 'worksheet' as const, worksheetPath: 'worksheets/Stale.yssbi-worksheet' },
    { kind: 'node' as const, id: 'node-old', graphPath: 'events/Old.yssbi-event' },
    { kind: 'function' as const, path: 'functions/Old.yssbi-function' },
  ])('clears stale replacement detail focus $kind', (detailFocus) => {
    const plan = buildAuthoritativeProjectLoadPlan(
      { path: null, databases: {}, index: emptyIndex() },
      {
        databases: {},
        layoutNodes: createInitialWorkbenchNodes(),
        editorTabs: { registry: {}, placements: {} },
        recentEditorGroupIds: [],
        detailFocus,
      },
      {
        ...defaultAuthoritativeProjectLoadPlanDependencies,
        validateCoordinatorStart: () => undefined,
      },
    );

    expect(plan.storeState.detailFocus).toBeNull();
  });
});
