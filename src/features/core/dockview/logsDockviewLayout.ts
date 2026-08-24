import { Orientation, type SerializedDockview } from 'dockview-react';

import {
  isLogDomainId,
  LOG_DOMAIN_ORDER,
  logDomainPanelId,
  logDomainTitle,
  type LogDomainId,
} from '@/features/core/log/logDomains';

export const LOGS_DOCKVIEW_COMPONENT_ID = 'LogDomainPanel' as const;
export const LOGS_DOCKVIEW_DEFAULT_GROUP_ID = 'logs-domain-group';

export interface LogsDockviewPanelParams {
  readonly domain: LogDomainId;
}

const EDGE_POSITIONS = ['top', 'bottom', 'left', 'right'] as const;

type UnknownRecord = Record<string, unknown>;

interface TopologyValidationState {
  readonly panelIds: ReadonlySet<string>;
  readonly referencedPanelIds: Set<string>;
  readonly groupIds: Set<string>;
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

function validateSerializedPanel(
  panelId: string,
  value: unknown,
  domains: Set<LogDomainId>,
): boolean {
  if (!isRecord(value) || !hasOnlyKeys(value, [
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
  if (value.id !== panelId || value.contentComponent !== LOGS_DOCKVIEW_COMPONENT_ID) return false;
  if (value.tabComponent !== undefined && typeof value.tabComponent !== 'string') return false;
  if (value.title !== undefined && typeof value.title !== 'string') return false;
  if (value.renderer !== undefined && typeof value.renderer !== 'string') return false;
  if (!isOptionalFiniteNumber(value.minimumWidth)
    || !isOptionalFiniteNumber(value.minimumHeight)
    || !isOptionalFiniteNumber(value.maximumWidth)
    || !isOptionalFiniteNumber(value.maximumHeight)
    || !isOptionalBoolean(value.pinned)) return false;
  if (!isRecord(value.params)
    || !hasOnlyKeys(value.params, ['domain'])
    || !isLogDomainId(value.params.domain)
    || panelId !== logDomainPanelId(value.params.domain)
    || domains.has(value.params.domain)) return false;

  domains.add(value.params.domain);
  return true;
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

function validateGroup(
  candidate: unknown,
  state: TopologyValidationState,
): boolean {
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

function validateEdgeGroups(
  candidate: unknown,
  state: TopologyValidationState,
): boolean {
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

export function createDefaultLogsDockviewLayout(): SerializedDockview {
  const panelIds = LOG_DOMAIN_ORDER.map(logDomainPanelId);
  return {
    grid: {
      root: {
        type: 'branch',
        data: [{
          type: 'leaf',
          data: {
            id: LOGS_DOCKVIEW_DEFAULT_GROUP_ID,
            views: panelIds,
            activeView: panelIds[0],
          },
        }],
      },
      height: 600,
      width: 1000,
      orientation: Orientation.HORIZONTAL,
    },
    panels: Object.fromEntries(LOG_DOMAIN_ORDER.map((domain) => {
      const id = logDomainPanelId(domain);
      return [id, {
        id,
        contentComponent: LOGS_DOCKVIEW_COMPONENT_ID,
        title: logDomainTitle(domain),
        params: { domain },
      }];
    })),
    activeGroup: LOGS_DOCKVIEW_DEFAULT_GROUP_ID,
    floatingGroups: [],
    popoutGroups: [],
  };
}

export const DEFAULT_LOGS_DOCKVIEW_LAYOUT = createDefaultLogsDockviewLayout();

export function isValidLogsDockviewLayout(candidate: unknown): candidate is SerializedDockview {
  if (!isRecord(candidate) || !hasOnlyKeys(candidate, [
    'grid',
    'panels',
    'activeGroup',
    'floatingGroups',
    'popoutGroups',
    'edgeGroups',
  ])) return false;
  if (!isRecord(candidate.panels)) return false;
  if (candidate.floatingGroups !== undefined
    && (!Array.isArray(candidate.floatingGroups) || candidate.floatingGroups.length > 0)) return false;
  if (candidate.popoutGroups !== undefined
    && (!Array.isArray(candidate.popoutGroups) || candidate.popoutGroups.length > 0)) return false;

  const domains = new Set<LogDomainId>();
  for (const [panelId, panel] of Object.entries(candidate.panels)) {
    if (!isNonEmptyString(panelId) || !validateSerializedPanel(panelId, panel, domains)) return false;
  }

  const state: TopologyValidationState = {
    panelIds: new Set(Object.keys(candidate.panels)),
    referencedPanelIds: new Set(),
    groupIds: new Set(),
  };
  if (!validateGrid(candidate.grid, state) || !validateEdgeGroups(candidate.edgeGroups, state)) {
    return false;
  }
  if (state.referencedPanelIds.size !== state.panelIds.size) return false;
  return candidate.activeGroup === undefined
    || (isNonEmptyString(candidate.activeGroup) && state.groupIds.has(candidate.activeGroup));
}
