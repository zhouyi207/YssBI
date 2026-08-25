import { Orientation, type SerializedDockview } from 'dockview-react';
import { describe, expect, it } from 'vitest';

import {
  orderWorkbenchPanelIdsForReset,
  WORKBENCH_EDGE_GROUP_IDS,
  WORKBENCH_EDGE_SIZES,
  WORKBENCH_HOME_EDGE,
  WORKBENCH_RESET_BUCKET_ORDER,
} from './workbenchDockviewDefaults';

const SERIALIZED_PANEL_IDS = [
  'left-a',
  'left-b',
  'top-a',
  'grid-a',
  'grid-b',
  'grid-c',
  'right-a',
  'bottom-a',
  'bottom-b',
] as const;

function serializedLayout(): SerializedDockview {
  return {
    grid: {
      root: {
        type: 'branch',
        size: 1200,
        data: [
          {
            type: 'leaf',
            size: 800,
            data: {
              id: 'grid-first',
              views: ['grid-a', 'grid-b'],
              activeView: 'grid-a',
            },
          },
          {
            type: 'branch',
            size: 400,
            data: [
              {
                type: 'leaf',
                size: 800,
                data: {
                  id: 'grid-second',
                  views: ['grid-c'],
                  activeView: 'grid-c',
                },
              },
            ],
          },
        ],
      },
      height: 800,
      width: 1200,
      orientation: Orientation.HORIZONTAL,
    },
    panels: Object.fromEntries(SERIALIZED_PANEL_IDS.map((id) => [
      id,
      { id, contentComponent: 'TestPanel' },
    ])),
    edgeGroups: {
      left: {
        size: 260,
        visible: true,
        group: {
          id: 'left-edge',
          views: ['left-a', 'left-b'],
          activeView: 'left-a',
        },
      },
      top: {
        size: 180,
        visible: true,
        group: {
          id: 'top-edge',
          views: ['top-a'],
          activeView: 'top-a',
        },
      },
      right: {
        size: 320,
        visible: true,
        group: {
          id: 'right-edge',
          views: ['right-a'],
          activeView: 'right-a',
        },
      },
      bottom: {
        size: 200,
        visible: true,
        collapsed: true,
        group: {
          id: 'bottom-edge',
          views: ['bottom-a', 'bottom-b'],
          activeView: 'bottom-a',
        },
      },
    },
  };
}

describe('workbench Dockview defaults', () => {
  it('defines panel homes and resets in edge/grid order even when bottom is collapsed', () => {
    expect(WORKBENCH_EDGE_GROUP_IDS).toEqual({
      left: 'workbench-edge-left',
      right: 'workbench-edge-right',
      bottom: 'workbench-edge-bottom',
    });
    expect(WORKBENCH_EDGE_SIZES).toEqual({ left: 292, right: 320, bottom: 200 });
    expect(WORKBENCH_HOME_EDGE).toEqual({
      project: 'left',
      nodes: 'left',
      data: 'left',
      commands: 'left',
      details: 'right',
      inspect: 'right',
      logs: 'bottom',
      output: 'bottom',
      diagnostics: 'bottom',
      result: 'right',
    });
    expect(WORKBENCH_RESET_BUCKET_ORDER).toEqual([
      'left',
      'top',
      'grid',
      'right',
      'bottom',
    ]);

    const layout = serializedLayout();
    expect(layout.edgeGroups?.bottom?.collapsed).toBe(true);
    expect(orderWorkbenchPanelIdsForReset(layout, SERIALIZED_PANEL_IDS)).toEqual(
      SERIALIZED_PANEL_IDS,
    );
  });

  it('appends live panels missing from serialization once', () => {
    const livePanelIds = [
      'left-a',
      'left-b',
      'grid-a',
      'grid-b',
      'grid-c',
      'right-a',
      'bottom-a',
      'bottom-b',
      'live-unserialized',
      'live-unserialized',
    ];

    expect(orderWorkbenchPanelIdsForReset(serializedLayout(), livePanelIds)).toEqual([
      'left-a',
      'left-b',
      'grid-a',
      'grid-b',
      'grid-c',
      'right-a',
      'bottom-a',
      'bottom-b',
      'live-unserialized',
    ]);
  });
});
