import type {
  GraphOutputRefDto,
  ResultPresentation,
} from '@/shared/types/dto/result';
import type { LayoutTab } from '@/shared/types/layout/layout';

export const WORKBENCH_VIEW_IDS = [
  'resources',
  'details',
  'inspect',
  'logs',
  'output',
] as const;

export type WorkbenchViewId = (typeof WORKBENCH_VIEW_IDS)[number];
export type EditorResourceKind = 'event' | 'function' | 'worksheet';
export type WorkbenchComponentId =
  | 'GraphEditor'
  | 'WorksheetEditor'
  | 'Resources'
  | 'Details'
  | 'Inspect'
  | 'Result'
  | 'Logs'
  | 'Output';

export type EditorPanelMetadata = {
  readonly role: 'editor';
  readonly resourceRef: string;
  readonly resourceKind: EditorResourceKind;
  readonly pinned?: boolean;
  readonly sticky?: boolean;
};

export type ViewPanelMetadata = {
  readonly role: 'view';
  readonly viewId: WorkbenchViewId;
};

export type ResultPanelMetadata = {
  readonly role: 'result';
  readonly resultKey: string;
  readonly resultId: string;
  readonly title: string;
  readonly presentation: ResultPresentation;
  readonly source: GraphOutputRefDto | null;
};

export type WorkbenchPanelMetadata =
  | EditorPanelMetadata
  | ViewPanelMetadata
  | ResultPanelMetadata;

export interface WorkbenchPanelParams extends Record<string, unknown> {
  readonly metadata: WorkbenchPanelMetadata;
}

const EDITOR_RESOURCE_KINDS = new Set<EditorResourceKind>([
  'event',
  'function',
  'worksheet',
]);
const WORKBENCH_VIEW_ID_SET = new Set<WorkbenchViewId>(WORKBENCH_VIEW_IDS);
const RESULT_PLOT_KINDS = new Set([
  'scatter',
  'line',
  'plot',
  'ecdf',
  'kde',
  'histogram',
  'correlation',
  'correlogram',
]);
const RESULT_REPORT_KINDS = new Set([
  'olsSummary',
  'binarySummary',
  'iv2slsSummary',
  'ivLimlSummary',
  'praisSummary',
  'varSummary',
  'varSoc',
  'panelSummary',
  'panelDid',
  'dfAdfSummary',
  'dfAdfSummaryList',
  'vecSummary',
  'vecRankSummary',
]);

const COMPONENT_BY_VIEW_ID: Readonly<Record<WorkbenchViewId, WorkbenchComponentId>> = {
  resources: 'Resources',
  details: 'Details',
  inspect: 'Inspect',
  logs: 'Logs',
  output: 'Output',
};

type UnknownRecord = Record<string, unknown>;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function hasKnownKeys(
  value: UnknownRecord,
  requiredKeys: readonly string[],
  optionalKeys: readonly string[] = [],
): boolean {
  const allowedKeys = new Set([...requiredKeys, ...optionalKeys]);
  return requiredKeys.every((key) => Object.prototype.hasOwnProperty.call(value, key))
    && Object.keys(value).every((key) => allowedKeys.has(key));
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0;
}

function isResultPresentation(value: unknown): value is ResultPresentation {
  if (!isRecord(value) || typeof value.kind !== 'string') return false;

  switch (value.kind) {
    case 'inspector':
      return hasKnownKeys(value, ['kind']);
    case 'plot':
      return hasKnownKeys(value, ['kind', 'chart'])
        && typeof value.chart === 'string'
        && RESULT_PLOT_KINDS.has(value.chart);
    case 'report':
      return hasKnownKeys(value, ['kind', 'report'])
        && typeof value.report === 'string'
        && RESULT_REPORT_KINDS.has(value.report);
    default:
      return false;
  }
}

function isPortAddress(value: unknown): boolean {
  if (!isRecord(value) || typeof value.kind !== 'string') return false;

  if (value.kind === 'declared') {
    return hasKnownKeys(value, ['kind', 'nodeId', 'portKey'])
      && isNonEmptyString(value.nodeId)
      && isNonEmptyString(value.portKey);
  }
  if (value.kind === 'instance') {
    return hasKnownKeys(value, ['kind', 'nodeId', 'templateKey', 'instanceId'])
      && isNonEmptyString(value.nodeId)
      && isNonEmptyString(value.templateKey)
      && isNonEmptyString(value.instanceId);
  }
  return false;
}

function isGraphOutputRef(value: unknown): value is GraphOutputRefDto {
  return isRecord(value)
    && hasKnownKeys(value, ['graphPath', 'port'])
    && isNonEmptyString(value.graphPath)
    && isPortAddress(value.port);
}

export function isWorkbenchPanelMetadata(value: unknown): value is WorkbenchPanelMetadata {
  if (!isRecord(value) || typeof value.role !== 'string') return false;

  switch (value.role) {
    case 'editor':
      return hasKnownKeys(
        value,
        ['role', 'resourceRef', 'resourceKind'],
        ['pinned', 'sticky'],
      )
        && isNonEmptyString(value.resourceRef)
        && typeof value.resourceKind === 'string'
        && EDITOR_RESOURCE_KINDS.has(value.resourceKind as EditorResourceKind)
        && (value.pinned === undefined || typeof value.pinned === 'boolean')
        && (value.sticky === undefined || typeof value.sticky === 'boolean');
    case 'view':
      return hasKnownKeys(value, ['role', 'viewId'])
        && typeof value.viewId === 'string'
        && WORKBENCH_VIEW_ID_SET.has(value.viewId as WorkbenchViewId);
    case 'result':
      return hasKnownKeys(value, [
        'role',
        'resultKey',
        'resultId',
        'title',
        'presentation',
        'source',
      ])
        && isNonEmptyString(value.resultKey)
        && isNonEmptyString(value.resultId)
        && typeof value.title === 'string'
        && isResultPresentation(value.presentation)
        && (value.source === null || isGraphOutputRef(value.source));
    default:
      return false;
  }
}

export function componentForWorkbenchMetadata(
  metadata: WorkbenchPanelMetadata,
): WorkbenchComponentId {
  if (metadata.role === 'editor') {
    return metadata.resourceKind === 'worksheet' ? 'WorksheetEditor' : 'GraphEditor';
  }
  if (metadata.role === 'result') return 'Result';
  return COMPONENT_BY_VIEW_ID[metadata.viewId];
}

export function layoutTabFromEditorMetadata(metadata: EditorPanelMetadata): LayoutTab {
  return {
    id: metadata.resourceRef,
    type: metadata.resourceKind,
    component: metadata.resourceKind === 'worksheet' ? 'WorksheetEditor' : 'GraphEditor',
    ...(metadata.pinned === undefined ? {} : { pinned: metadata.pinned }),
    ...(metadata.sticky === undefined ? {} : { sticky: metadata.sticky }),
  };
}
