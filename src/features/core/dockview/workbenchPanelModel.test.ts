import { describe, expect, it } from 'vitest';

import type { GraphOutputRefDto } from '@/shared/types/domain/result';
import {
  componentForWorkbenchMetadata,
  isWorkbenchPanelMetadata,
  layoutTabFromEditorMetadata,
  type ResultPanelMetadata,
  type ViewPanelMetadata,
  type WorkbenchComponentId,
} from './workbenchPanelModel';

describe('workbench panel metadata', () => {
  it('keeps canonical identity, component selection, and editor projection in one place', () => {
    const metadata = {
      role: 'editor',
      resourceRef: 'events/Main.yssbi-event',
      resourceKind: 'event',
      pinned: false,
      sticky: true,
    } as const;

    expect(isWorkbenchPanelMetadata(metadata)).toBe(true);
    expect(componentForWorkbenchMetadata(metadata)).toBe('GraphEditor');
    expect(layoutTabFromEditorMetadata(metadata)).toEqual({
      id: 'events/Main.yssbi-event',
      type: 'event',
      component: 'GraphEditor',
      pinned: false,
      sticky: true,
    });
    expect(metadata).not.toHaveProperty('layoutTab');

    expect(componentForWorkbenchMetadata({
      role: 'editor',
      resourceRef: 'worksheets/Model.yssbi-worksheet',
      resourceKind: 'worksheet',
    })).toBe('WorksheetEditor');
  });

  it('accepts every view and canonical result presentation/source variant', () => {
    const viewCases: readonly {
      metadata: ViewPanelMetadata;
      component: WorkbenchComponentId;
    }[] = [
      { metadata: { role: 'view', viewId: 'project' }, component: 'Project' },
      { metadata: { role: 'view', viewId: 'nodes' }, component: 'Nodes' },
      { metadata: { role: 'view', viewId: 'data' }, component: 'Data' },
      { metadata: { role: 'view', viewId: 'commands' }, component: 'Commands' },
      { metadata: { role: 'view', viewId: 'details' }, component: 'Details' },
      { metadata: { role: 'view', viewId: 'inspect' }, component: 'Inspect' },
      { metadata: { role: 'view', viewId: 'logs' }, component: 'Logs' },
      { metadata: { role: 'view', viewId: 'output' }, component: 'Output' },
      { metadata: { role: 'view', viewId: 'diagnostics' }, component: 'Diagnostics' },
    ];
    for (const { metadata: view, component } of viewCases) {
      expect(isWorkbenchPanelMetadata(view)).toBe(true);
      expect(componentForWorkbenchMetadata(view)).toBe(component);
    }

    const declaredSource: GraphOutputRefDto = {
      graphPath: 'events/Main.yssbi-event',
      port: {
        kind: 'declared',
        nodeId: '11111111-1111-4111-8111-111111111111',
        portKey: 'result',
      },
    };
    const instanceSource: GraphOutputRefDto = {
      graphPath: 'functions/Model.yssbi-function',
      port: {
        kind: 'instance',
        nodeId: '22222222-2222-4222-8222-222222222222',
        templateKey: 'columns',
        instanceId: '33333333-3333-4333-8333-333333333333',
      },
    };
    const results: readonly ResultPanelMetadata[] = [
      {
        role: 'result',
        resultKey: 'inspector-result',
        resultId: '41',
        title: 'Inspector result',
        presentation: { kind: 'inspector' },
        source: null,
      },
      {
        role: 'result',
        resultKey: 'plot-result',
        resultId: '42',
        title: 'Plot result',
        presentation: { kind: 'plot', chart: 'scatter' },
        source: declaredSource,
      },
      {
        role: 'result',
        resultKey: 'report-result',
        resultId: '43',
        title: 'Report result',
        presentation: { kind: 'report', report: 'olsSummary' },
        source: instanceSource,
      },
    ];
    for (const result of results) {
      expect(isWorkbenchPanelMetadata(result)).toBe(true);
      expect(componentForWorkbenchMetadata(result)).toBe('Result');
    }
  });

  it('rejects empty, unknown, and obsolete metadata structures', () => {
    const invalidMetadata: unknown[] = [
      {
        role: 'editor',
        resourceRef: '',
        resourceKind: 'event',
      },
      {
        role: 'editor',
        resourceRef: 'settings',
        resourceKind: 'project',
      },
      {
        role: 'editor',
        resourceRef: 'settings',
        resourceKind: 'setting',
      },
      {
        role: 'editor',
        resourceRef: 'events/Main.yssbi-event',
        resourceKind: 'event',
        legacyId: 'old',
      },
      { role: 'view', viewId: 'result' },
      { role: 'view', viewId: 'settings' },
      { role: 'unknown', viewId: 'obsolete-view' },
      {
        role: 'result',
        resultKey: '',
        resultId: '42',
        title: 'Result',
        presentation: { kind: 'inspector' },
        source: null,
      },
      {
        role: 'result',
        resultKey: 'result-key',
        resultId: '',
        title: 'Result',
        presentation: { kind: 'inspector' },
        source: null,
      },
    ];

    for (const metadata of invalidMetadata) {
      expect(isWorkbenchPanelMetadata(metadata)).toBe(false);
    }
  });
});
