import { describe, expect, it } from 'vitest';
import type { SidebarPanelModel } from '@/features/core/sidebar';
import { flattenSidebarPanelModel } from './sidebarRenderRows';

const emptyGraphs: SidebarPanelModel = {
  sections: [
    {
      key: 'graphsEvent',
      label: 'Event',
      expanded: true,
      rows: [],
      emptyMessage: 'No events',
    },
    {
      key: 'graphsFunction',
      label: 'Function',
      expanded: false,
      rows: [],
      emptyMessage: 'No functions',
    },
  ],
};

describe('flattenSidebarPanelModel', () => {
  it('emits section empty rows only for expanded empty sections', () => {
    expect(flattenSidebarPanelModel(emptyGraphs)).toEqual([
      {
        kind: 'section',
        rowKey: 'section:graphsEvent',
        sectionKey: 'graphsEvent',
        level: 0,
        label: 'Event',
        expanded: true,
      },
      {
        kind: 'sectionEmpty',
        rowKey: 'section-empty:graphsEvent',
        sectionKey: 'graphsEvent',
        level: 1,
        message: 'No events',
      },
      {
        kind: 'section',
        rowKey: 'section:graphsFunction',
        sectionKey: 'graphsFunction',
        level: 0,
        label: 'Function',
        expanded: false,
      },
    ]);
  });

  it('places populated rows after their expanded section header', () => {
    const model: SidebarPanelModel = {
      sections: [
        {
          key: 'dataData',
          label: 'Data',
          expanded: true,
          emptyMessage: 'No data',
          rows: [
            {
              kind: 'database',
              rowKey: 'database:db-1',
              level: 1,
              id: 'db-1',
              name: 'Sales',
              data: { name: 'Sales' },
            },
          ],
        },
      ],
    };

    expect(flattenSidebarPanelModel(model).map((row) => row.kind)).toEqual([
      'section',
      'database',
    ]);
  });

  it('does not synthesize a placeholder without an empty message', () => {
    const model: SidebarPanelModel = {
      sections: [{ key: 'dataData', label: 'Data', expanded: true, rows: [] }],
    };
    expect(flattenSidebarPanelModel(model)).toHaveLength(1);
  });
});
