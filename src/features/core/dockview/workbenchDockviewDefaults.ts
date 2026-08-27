import type { SerializedDockview } from 'dockview-react';

import { WORKBENCH_ACTIVITY_VIEW_IDS } from './workbenchPanelModel';

export const WORKBENCH_ACTIVITY_GROUP_ID = 'workbench-edge-left';

export const WORKBENCH_EDGE_GROUP_IDS = {
  left: 'workbench-edge-left',
  right: 'workbench-edge-right',
  bottom: 'workbench-edge-bottom',
} as const;

export const WORKBENCH_EDGE_SIZES = {
  left: 292,
  right: 320,
  bottom: 200,
} as const;

export const WORKBENCH_HOME_EDGE = {
  project: 'left',
  nodes: 'left',
  data: 'left',
  commands: 'left',
  details: 'right',
  assistant: 'right',
  inspect: 'right',
  logs: 'bottom',
  output: 'bottom',
  result: 'right',
  diagnostics: 'bottom',
} as const;

export const WORKBENCH_ACTIVITY_DEFAULT_ORDER = WORKBENCH_ACTIVITY_VIEW_IDS;

export const WORKBENCH_RESET_BUCKET_ORDER = [
  'left',
  'top',
  'grid',
  'right',
  'bottom',
] as const;

type SerializedGridNode = SerializedDockview['grid']['root'];
type EdgePosition = Exclude<(typeof WORKBENCH_RESET_BUCKET_ORDER)[number], 'grid'>;

type UnknownRecord = Record<string, unknown>;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function readViews(value: unknown): readonly string[] {
  if (!isRecord(value) || !Array.isArray(value.views)) return [];
  return value.views.filter((view): view is string => typeof view === 'string');
}

function isSerializedGridNode(value: unknown): value is SerializedGridNode {
  return isRecord(value)
    && (value.type === 'leaf' || value.type === 'branch')
    && Object.prototype.hasOwnProperty.call(value, 'data');
}

function visitGridViews(
  node: SerializedGridNode,
  visit: (views: readonly string[]) => void,
): void {
  if (node.type === 'leaf') {
    visit(readViews(node.data));
    return;
  }
  if (node.type !== 'branch' || !Array.isArray(node.data)) return;

  for (const child of node.data) {
    if (isSerializedGridNode(child)) visitGridViews(child, visit);
  }
}

function edgeViews(layout: SerializedDockview, position: EdgePosition): readonly string[] {
  return readViews(layout.edgeGroups?.[position]?.group);
}

export function orderWorkbenchPanelIdsForReset(
  layout: SerializedDockview,
  livePanelIds: readonly string[],
): readonly string[] {
  const liveIds = new Set(livePanelIds);
  const seen = new Set<string>();
  const ordered: string[] = [];
  const appendViews = (views: readonly string[]): void => {
    for (const panelId of views) {
      if (!liveIds.has(panelId) || seen.has(panelId)) continue;
      seen.add(panelId);
      ordered.push(panelId);
    }
  };

  for (const bucket of WORKBENCH_RESET_BUCKET_ORDER) {
    if (bucket === 'grid') {
      visitGridViews(layout.grid.root, appendViews);
    } else {
      appendViews(edgeViews(layout, bucket));
    }
  }

  appendViews(livePanelIds);
  return ordered;
}
