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
              selectedTabIds: ['settings'],
            },
          },
        },
        recentEditorGroupIds: ['split_editor'],
      },
      {
        ...defaultAuthoritativeProjectLoadPlanDependencies,
        validateCoordinatorStart: () => undefined,
      },
    );

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
          selectedTabIds: [],
        },
      },
    });
  });
});
