import type { SerializedDockview } from 'dockview-react';

import {
  SIDEBAR_TAB_IDS,
  type SidebarTabId,
} from '@/features/core/workbench/workbenchTypes';
import { isValidLogsDockviewLayout } from './logsDockviewLayout';
import {
  componentForWorkbenchMetadata,
  isWorkbenchPanelMetadata,
  type WorkbenchPanelMetadata,
} from './workbenchPanelModel';

export interface PersistedWorkbenchLayout {
  readonly root: SerializedDockview;
  readonly nested: { readonly logs: SerializedDockview };
  readonly preferences: { readonly sidebarCurrentTab: SidebarTabId };
}

export type ParsedLayoutPart<T> =
  | { readonly status: 'valid'; readonly value: T }
  | { readonly status: 'invalid' };

export interface ParsedPersistedWorkbenchLayout {
  readonly root: ParsedLayoutPart<SerializedDockview>;
  readonly logs: ParsedLayoutPart<SerializedDockview>;
  readonly preferences: { readonly sidebarCurrentTab: SidebarTabId };
}

const SIDEBAR_TAB_ID_SET = new Set<SidebarTabId>(SIDEBAR_TAB_IDS);
const EDGE_POSITIONS = ['top', 'bottom', 'left', 'right'] as const;

type UnknownRecord = Record<string, unknown>;
type GridNode = SerializedDockview['grid']['root'];
type GridWithMaximizedNode = SerializedDockview['grid'] & {
  maximizedNode?: unknown;
};

interface TopologyValidationState {
  readonly panelIds: ReadonlySet<string>;
  readonly referencedPanelIds: Set<string>;
  readonly groupIds: Set<string>;
}

interface PrunedGridNode {
  readonly node: GridNode | undefined;
  readonly topologyChanged: boolean;
}

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function hasOnlyKeys(value: UnknownRecord, allowed: readonly string[]): boolean {
  const allowedKeys = new Set(allowed);
  return Object.keys(value).every((key) => allowedKeys.has(key));
}

function hasExactKeys(value: UnknownRecord, expected: readonly string[]): boolean {
  return Object.keys(value).length === expected.length
    && expected.every((key) => Object.prototype.hasOwnProperty.call(value, key));
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0;
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}

function isOptionalBoolean(value: unknown): boolean {
  return value === undefined || typeof value === 'boolean';
}

function isOptionalFiniteNumber(value: unknown): boolean {
  return value === undefined || isFiniteNumber(value);
}

function readMetadata(panel: unknown): WorkbenchPanelMetadata | undefined {
  if (!isRecord(panel) || !isRecord(panel.params)) return undefined;
  return isWorkbenchPanelMetadata(panel.params.metadata)
    ? panel.params.metadata
    : undefined;
}

function isTransientWorkbenchMetadata(metadata: WorkbenchPanelMetadata): boolean {
  return metadata.role === 'result'
    || (metadata.role === 'view'
      && (metadata.viewId === 'details' || metadata.viewId === 'inspect'));
}

function isProjectScopedWorkbenchMetadata(metadata: WorkbenchPanelMetadata): boolean {
  return metadata.role === 'editor' || isTransientWorkbenchMetadata(metadata);
}

function validatePanelShape(panelId: string, panel: unknown): panel is UnknownRecord {
  if (!isRecord(panel) || !hasOnlyKeys(panel, [
    'id',
    'contentComponent',
    'tabComponent',
    'title',
    'renderer',
    'params',
    'minimumWidth',
    'minimumHeight',
    'maximumWidth',
    'maximumHeight',
    'pinned',
  ])) return false;
  if (panel.id !== panelId || !isNonEmptyString(panel.contentComponent)) return false;
  if (panel.tabComponent !== undefined && typeof panel.tabComponent !== 'string') return false;
  if (panel.title !== undefined && typeof panel.title !== 'string') return false;
  if (panel.renderer !== undefined && typeof panel.renderer !== 'string') return false;
  return isRecord(panel.params)
    && isOptionalFiniteNumber(panel.minimumWidth)
    && isOptionalFiniteNumber(panel.minimumHeight)
    && isOptionalFiniteNumber(panel.maximumWidth)
    && isOptionalFiniteNumber(panel.maximumHeight)
    && isOptionalBoolean(panel.pinned);
}

function validateRootPanels(candidate: unknown): ReadonlySet<string> | undefined {
  if (!isRecord(candidate)) return undefined;

  const panelIds = new Set<string>();
  const singletonViews = new Set<string>();
  for (const [panelId, panel] of Object.entries(candidate)) {
    if (!isNonEmptyString(panelId) || !validatePanelShape(panelId, panel)) return undefined;
    const metadata = readMetadata(panel);
    if (!metadata
      || isTransientWorkbenchMetadata(metadata)
      || panel.contentComponent !== componentForWorkbenchMetadata(metadata)) return undefined;
    if (metadata.role === 'view') {
      if (singletonViews.has(metadata.viewId)) return undefined;
      singletonViews.add(metadata.viewId);
    }
    panelIds.add(panelId);
  }
  return panelIds;
}

function validateTabGroups(value: unknown, groupViews: ReadonlySet<string>): boolean {
  if (value === undefined) return true;
  if (!Array.isArray(value)) return false;

  const groupIds = new Set<string>();
  const groupedPanels = new Set<string>();
  for (const candidate of value) {
    if (!isRecord(candidate) || !hasOnlyKeys(candidate, [
      'id',
      'label',
      'color',
      'collapsed',
      'panelIds',
      'componentParams',
    ])) return false;
    if (!isNonEmptyString(candidate.id)
      || groupIds.has(candidate.id)
      || typeof candidate.collapsed !== 'boolean'
      || !Array.isArray(candidate.panelIds)) return false;
    if (candidate.label !== undefined && typeof candidate.label !== 'string') return false;
    if (candidate.color !== undefined && typeof candidate.color !== 'string') return false;
    if (candidate.componentParams !== undefined && !isRecord(candidate.componentParams)) return false;

    groupIds.add(candidate.id);
    for (const panelId of candidate.panelIds) {
      if (!isNonEmptyString(panelId)
        || !groupViews.has(panelId)
        || groupedPanels.has(panelId)) return false;
      groupedPanels.add(panelId);
    }
  }
  return true;
}

function validateGroup(candidate: unknown, state: TopologyValidationState): boolean {
  if (!isRecord(candidate) || !hasOnlyKeys(candidate, [
    'id',
    'views',
    'activeView',
    'locked',
    'hideHeader',
    'headerPosition',
    'skipSetActive',
    'constraints',
    'initialWidth',
    'initialHeight',
    'tabGroups',
  ])) return false;
  if (!isNonEmptyString(candidate.id)
    || state.groupIds.has(candidate.id)
    || !Array.isArray(candidate.views)) return false;
  if (candidate.locked !== undefined
    && typeof candidate.locked !== 'boolean'
    && candidate.locked !== 'no-drop-target') return false;
  if (!isOptionalBoolean(candidate.hideHeader)
    || !isOptionalBoolean(candidate.skipSetActive)
    || !isOptionalFiniteNumber(candidate.initialWidth)
    || !isOptionalFiniteNumber(candidate.initialHeight)) return false;
  if (candidate.headerPosition !== undefined
    && !['top', 'bottom', 'left', 'right'].includes(String(candidate.headerPosition))) return false;
  if (candidate.constraints !== undefined && !isRecord(candidate.constraints)) return false;

  const views = new Set<string>();
  for (const panelId of candidate.views) {
    if (!isNonEmptyString(panelId)
      || views.has(panelId)
      || !state.panelIds.has(panelId)
      || state.referencedPanelIds.has(panelId)) return false;
    views.add(panelId);
    state.referencedPanelIds.add(panelId);
  }
  if (views.size > 0) {
    if (!isNonEmptyString(candidate.activeView) || !views.has(candidate.activeView)) return false;
  } else if (candidate.activeView !== undefined && candidate.activeView !== '') {
    return false;
  }
  if (!validateTabGroups(candidate.tabGroups, views)) return false;

  state.groupIds.add(candidate.id);
  return true;
}

function validateGridNode(
  candidate: unknown,
  state: TopologyValidationState,
): boolean {
  if (!isRecord(candidate)
    || !hasOnlyKeys(candidate, ['type', 'data', 'size', 'visible'])
    || (candidate.type !== 'leaf' && candidate.type !== 'branch')
    || !isOptionalFiniteNumber(candidate.size)
    || !isOptionalBoolean(candidate.visible)) return false;

  if (candidate.type === 'leaf') return validateGroup(candidate.data, state);
  if (!Array.isArray(candidate.data)) return false;
  return candidate.data.every((child) => validateGridNode(child, state));
}

function validateMaximizedNode(candidate: unknown, root: unknown): boolean {
  if (candidate === undefined) return true;
  if (!isRecord(candidate)
    || !hasExactKeys(candidate, ['location'])
    || !Array.isArray(candidate.location)
    || candidate.location.length === 0) return false;

  let node = root;
  for (const index of candidate.location) {
    if (typeof index !== 'number'
      || !Number.isFinite(index)
      || !Number.isInteger(index)
      || index < 0
      || !isRecord(node)
      || node.type !== 'branch'
      || !Array.isArray(node.data)
      || index >= node.data.length) return false;
    node = node.data[index];
  }
  return isRecord(node) && node.type === 'leaf';
}

function validateGrid(candidate: unknown, state: TopologyValidationState): boolean {
  return isRecord(candidate)
    && hasOnlyKeys(candidate, ['root', 'height', 'width', 'orientation', 'maximizedNode'])
    && isFiniteNumber(candidate.height)
    && isFiniteNumber(candidate.width)
    && (candidate.orientation === 'HORIZONTAL' || candidate.orientation === 'VERTICAL')
    && isRecord(candidate.root)
    && candidate.root.type === 'branch'
    && Array.isArray(candidate.root.data)
    && validateGridNode(candidate.root, state)
    && validateMaximizedNode(candidate.maximizedNode, candidate.root);
}

function validateEdgeGroups(candidate: unknown, state: TopologyValidationState): boolean {
  if (candidate === undefined) return true;
  if (!isRecord(candidate) || !hasOnlyKeys(candidate, EDGE_POSITIONS)) return false;

  for (const position of EDGE_POSITIONS) {
    const edge = candidate[position];
    if (edge === undefined) continue;
    if (!isRecord(edge) || !hasOnlyKeys(edge, [
      'size',
      'visible',
      'collapsed',
      'group',
      'autoHide',
      'autoReveal',
      'minimumSize',
      'maximumSize',
      'collapsedSize',
    ])) return false;
    if (!isFiniteNumber(edge.size)
      || typeof edge.visible !== 'boolean'
      || !isOptionalBoolean(edge.collapsed)
      || !isOptionalBoolean(edge.autoHide)
      || !isOptionalBoolean(edge.autoReveal)
      || !isOptionalFiniteNumber(edge.minimumSize)
      || !isOptionalFiniteNumber(edge.maximumSize)
      || !isOptionalFiniteNumber(edge.collapsedSize)
      || !isRecord(edge.group)
      || !Array.isArray(edge.group.views)
      || edge.group.views.length === 0
      || !validateGroup(edge.group, state)) return false;
  }
  return true;
}

function isValidRootLayout(candidate: unknown): candidate is SerializedDockview {
  if (!isRecord(candidate) || !hasOnlyKeys(candidate, [
    'grid',
    'panels',
    'activeGroup',
    'floatingGroups',
    'popoutGroups',
    'edgeGroups',
  ])) return false;
  if (candidate.floatingGroups !== undefined
    && (!Array.isArray(candidate.floatingGroups) || candidate.floatingGroups.length > 0)) return false;
  if (candidate.popoutGroups !== undefined
    && (!Array.isArray(candidate.popoutGroups) || candidate.popoutGroups.length > 0)) return false;

  const panelIds = validateRootPanels(candidate.panels);
  if (!panelIds) return false;
  const state: TopologyValidationState = {
    panelIds,
    referencedPanelIds: new Set(),
    groupIds: new Set(),
  };
  if (!validateGrid(candidate.grid, state) || !validateEdgeGroups(candidate.edgeGroups, state)) {
    return false;
  }
  if (state.referencedPanelIds.size !== panelIds.size) return false;
  return candidate.activeGroup === undefined
    || (isNonEmptyString(candidate.activeGroup) && state.groupIds.has(candidate.activeGroup));
}

function visitGridGroups(node: unknown, visit: (group: UnknownRecord) => void): void {
  if (!isRecord(node)) return;
  if (node.type === 'leaf') {
    if (isRecord(node.data)) visit(node.data);
    return;
  }
  if (node.type !== 'branch' || !Array.isArray(node.data)) return;
  node.data.forEach((child) => visitGridGroups(child, visit));
}

function visitAllGroups(layout: SerializedDockview, visit: (group: UnknownRecord) => void): void {
  visitGridGroups(layout.grid.root, visit);
  for (const position of EDGE_POSITIONS) {
    const group = layout.edgeGroups?.[position]?.group;
    if (isRecord(group)) visit(group);
  }
}

function removePanelRecords(layout: SerializedDockview, removed: ReadonlySet<string>): void {
  removed.forEach((panelId) => delete layout.panels[panelId]);
}

function removePanelReferencesFromGroup(
  group: UnknownRecord,
  removed: ReadonlySet<string>,
): void {
  if (Array.isArray(group.views)) {
    group.views = group.views.filter(
      (panelId): panelId is string => typeof panelId === 'string' && !removed.has(panelId),
    );
  }
  if (!Array.isArray(group.tabGroups)) return;
  group.tabGroups = group.tabGroups.flatMap((candidate) => {
    if (!isRecord(candidate) || !Array.isArray(candidate.panelIds)) return [candidate];
    const panelIds = candidate.panelIds.filter(
      (panelId): panelId is string => typeof panelId === 'string' && !removed.has(panelId),
    );
    candidate.panelIds = panelIds;
    return panelIds.length > 0 ? [candidate] : [];
  });
}

function removePanelReferencesFromGrid(
  layout: SerializedDockview,
  removed: ReadonlySet<string>,
): void {
  visitGridGroups(layout.grid.root, (group) => removePanelReferencesFromGroup(group, removed));
}

function removePanelReferencesFromEdges(
  layout: SerializedDockview,
  removed: ReadonlySet<string>,
): void {
  for (const position of EDGE_POSITIONS) {
    const group = layout.edgeGroups?.[position]?.group;
    if (isRecord(group)) removePanelReferencesFromGroup(group, removed);
  }
}

function repairActiveViews(layout: SerializedDockview): void {
  visitAllGroups(layout, (group) => {
    const views = Array.isArray(group.views)
      ? group.views.filter((panelId): panelId is string => typeof panelId === 'string')
      : [];
    if (typeof group.activeView === 'string'
      && group.activeView.length > 0
      && views.includes(group.activeView)) return;
    if (views.length > 0) group.activeView = views[0];
    else delete group.activeView;
  });
}

function pruneEmptyGridNode(node: GridNode): PrunedGridNode {
  if (node.type === 'leaf') {
    const group = node.data as unknown;
    const keep = isRecord(group) && Array.isArray(group.views) && group.views.length > 0;
    return { node: keep ? node : undefined, topologyChanged: !keep };
  }

  const serializedChildren = Array.isArray(node.data) ? node.data : [];
  const children: GridNode[] = [];
  let topologyChanged = !Array.isArray(node.data);
  for (const child of serializedChildren) {
    const pruned = pruneEmptyGridNode(child);
    topologyChanged ||= pruned.topologyChanged;
    if (pruned.node) children.push(pruned.node);
    else topologyChanged = true;
  }

  if (children.length === 0) return { node: undefined, topologyChanged: true };
  return { node: { ...node, data: children }, topologyChanged };
}

function removeEmptyGridLeaves(layout: SerializedDockview): boolean {
  const root = layout.grid.root;
  if (root.type !== 'branch' || !Array.isArray(root.data)) {
    const pruned = pruneEmptyGridNode(root);
    layout.grid.root = {
      type: 'branch',
      data: pruned.node ? [pruned.node] : [],
    };
    return true;
  }

  const children: GridNode[] = [];
  let topologyChanged = false;
  for (const child of root.data) {
    const pruned = pruneEmptyGridNode(child);
    topologyChanged ||= pruned.topologyChanged;
    if (pruned.node) children.push(pruned.node);
    else topologyChanged = true;
  }
  layout.grid.root = { ...root, data: children };
  return topologyChanged;
}

function removeEmptyEdgeGroups(layout: SerializedDockview): void {
  if (!layout.edgeGroups) return;
  for (const position of EDGE_POSITIONS) {
    const group = layout.edgeGroups[position]?.group;
    if (!isRecord(group) || !Array.isArray(group.views) || group.views.length === 0) {
      delete layout.edgeGroups[position];
    }
  }
  if (Object.keys(layout.edgeGroups).length === 0) delete layout.edgeGroups;
}

function repairActiveGroup(layout: SerializedDockview): void {
  const survivingGroupIds: string[] = [];
  visitAllGroups(layout, (group) => {
    if (isNonEmptyString(group.id)
      && Array.isArray(group.views)
      && group.views.length > 0) survivingGroupIds.push(group.id);
  });
  if (typeof layout.activeGroup === 'string'
    && survivingGroupIds.includes(layout.activeGroup)) return;
  if (survivingGroupIds.length > 0) layout.activeGroup = survivingGroupIds[0];
  else delete layout.activeGroup;
}

function cloneRootLayoutWithoutPanels(
  layout: SerializedDockview,
  shouldRemove: (metadata: WorkbenchPanelMetadata) => boolean,
): SerializedDockview {
  const clone = structuredClone(layout);
  const removed = new Set(
    Object.entries(clone.panels)
      .filter(([, panel]) => {
        const metadata = readMetadata(panel);
        return metadata !== undefined && shouldRemove(metadata);
      })
      .map(([panelId]) => panelId),
  );

  removePanelRecords(clone, removed);
  removePanelReferencesFromGrid(clone, removed);
  removePanelReferencesFromEdges(clone, removed);
  repairActiveViews(clone);
  const gridTopologyChanged = removeEmptyGridLeaves(clone);
  const grid = clone.grid as GridWithMaximizedNode;
  if (gridTopologyChanged || !validateMaximizedNode(grid.maximizedNode, grid.root)) {
    delete grid.maximizedNode;
  }
  removeEmptyEdgeGroups(clone);
  repairActiveGroup(clone);
  return clone;
}

export function workbenchLayoutStorageKey(label: string): string {
  return `yssbi-workbench-layout:${label || 'main'}`;
}

export function createPersistedWorkbenchLayout(
  root: SerializedDockview,
  logs: SerializedDockview,
  sidebarCurrentTab: SidebarTabId,
): PersistedWorkbenchLayout {
  return {
    root: prepareRootLayoutForPersistence(root),
    nested: { logs: structuredClone(logs) },
    preferences: { sidebarCurrentTab },
  };
}

export function parsePersistedWorkbenchLayout(
  candidate: unknown,
): ParsedPersistedWorkbenchLayout | null {
  if (!isRecord(candidate)
    || !hasExactKeys(candidate, ['root', 'nested', 'preferences'])
    || !isRecord(candidate.nested)
    || !hasExactKeys(candidate.nested, ['logs'])
    || !isRecord(candidate.preferences)
    || !hasExactKeys(candidate.preferences, ['sidebarCurrentTab'])
    || typeof candidate.preferences.sidebarCurrentTab !== 'string'
    || !SIDEBAR_TAB_ID_SET.has(candidate.preferences.sidebarCurrentTab as SidebarTabId)) {
    return null;
  }

  const sidebarCurrentTab = candidate.preferences.sidebarCurrentTab as SidebarTabId;
  return {
    root: isValidRootLayout(candidate.root)
      ? { status: 'valid', value: candidate.root }
      : { status: 'invalid' },
    logs: isValidLogsDockviewLayout(candidate.nested.logs)
      ? { status: 'valid', value: candidate.nested.logs }
      : { status: 'invalid' },
    preferences: { sidebarCurrentTab },
  };
}

export function prepareRootLayoutForPersistence(
  layout: SerializedDockview,
): SerializedDockview {
  return cloneRootLayoutWithoutPanels(layout, isTransientWorkbenchMetadata);
}

export function scrubProjectScopedRootLayout(
  layout: SerializedDockview,
): SerializedDockview {
  return cloneRootLayoutWithoutPanels(layout, isProjectScopedWorkbenchMetadata);
}
