import type {
  DockviewWillDropEvent,
  DockviewWillShowOverlayLocationEvent,
  IDockviewPanel,
} from 'dockview-react';

import {
  isWorkbenchActivityMetadata,
  isWorkbenchPanelMetadata,
  isWorkbenchPersistentViewMetadata,
  type WorkbenchPanelMetadata,
} from './workbenchPanelModel';
import {
  WORKBENCH_ACTIVITY_GROUP_ID,
  WORKBENCH_EDGE_GROUP_IDS,
} from './workbenchDockviewDefaults';

export { WORKBENCH_ACTIVITY_GROUP_ID } from './workbenchDockviewDefaults';

export type WorkbenchActivityDropEvent =
  | DockviewWillDropEvent
  | DockviewWillShowOverlayLocationEvent;

type ActivityPanelLike = Pick<IDockviewPanel, 'params'>;

type WorkbenchMoveTargetPosition = 'grid' | 'top' | 'bottom' | 'left' | 'right';

function panelMetadata(panel: ActivityPanelLike | undefined): WorkbenchPanelMetadata | undefined {
  const metadata = panel?.params && typeof panel.params === 'object'
    ? (panel.params as { metadata?: unknown }).metadata
    : undefined;
  return isWorkbenchPanelMetadata(metadata) ? metadata : undefined;
}

function activityMetadata(panel: ActivityPanelLike | undefined): WorkbenchPanelMetadata | undefined {
  const metadata = panelMetadata(panel);
  return metadata && isWorkbenchActivityMetadata(metadata) ? metadata : undefined;
}

function persistentSidebarMetadata(
  panel: ActivityPanelLike | undefined,
): WorkbenchPanelMetadata | undefined {
  const metadata = panelMetadata(panel);
  return metadata && isWorkbenchPersistentViewMetadata(metadata) ? metadata : undefined;
}

export function isWorkbenchActivityPanel(
  panel: ActivityPanelLike | undefined,
): boolean {
  return activityMetadata(panel) !== undefined;
}

function targetGroupId(
  event: Pick<WorkbenchActivityDropEvent, 'group' | 'panel'>,
): string | undefined {
  return event.group?.id ?? event.panel?.group.id;
}

function targetPosition(
  event: Pick<WorkbenchActivityDropEvent, 'group' | 'panel'>,
): WorkbenchMoveTargetPosition | undefined {
  const location = (event.group ?? event.panel?.group)?.api?.location;
  if (location?.type === 'grid') return 'grid';
  if (location?.type === 'edge') return location.position;
  return undefined;
}

function sourcePanel(
  event: Pick<WorkbenchActivityDropEvent, 'api' | 'getData'>,
): IDockviewPanel | undefined {
  const transfer = event.getData();
  if (!transfer || transfer.viewId !== event.api.id || transfer.panelId === null) return undefined;
  return event.api.getPanel(transfer.panelId);
}

function sourceGroupContainsPersistentView(
  event: Pick<WorkbenchActivityDropEvent, 'api' | 'getData'>,
): boolean {
  const transfer = event.getData();
  if (!transfer
    || transfer.viewId !== event.api.id
    || transfer.panelId !== null
    || transfer.tabGroupId !== undefined) return false;

  const sourceGroup = event.api.getGroup(transfer.groupId);
  return sourceGroup?.panels.some((panel) => (
    persistentSidebarMetadata(panel) !== undefined
  )) === true;
}

export function shouldAllowWorkbenchActivityDrop(
  event: WorkbenchActivityDropEvent,
): boolean {
  const transfer = event.getData();
  if (sourceGroupContainsPersistentView(event)) return false;

  const source = sourcePanel(event);
  const sourceIsPersistent = persistentSidebarMetadata(source) !== undefined;
  const sourceIsActivity = isWorkbenchActivityPanel(source)
    || transfer?.groupId === WORKBENCH_ACTIVITY_GROUP_ID;
  const targetIsActivity = targetGroupId(event) === WORKBENCH_ACTIVITY_GROUP_ID
    || isWorkbenchActivityPanel(event.panel);
  if (sourceIsPersistent) {
    return targetPosition(event) === 'right'
      || targetGroupId(event) === WORKBENCH_EDGE_GROUP_IDS.right;
  }
  if (!sourceIsActivity && !targetIsActivity) return true;

  const allowedReorder = transfer !== undefined
    && transfer.viewId === event.api.id
    && transfer.panelId !== null
    && transfer.tabGroupId === undefined
    && transfer.groupId === WORKBENCH_ACTIVITY_GROUP_ID
    && source !== undefined
    && isWorkbenchActivityPanel(source)
    && targetGroupId(event) === WORKBENCH_ACTIVITY_GROUP_ID
    && (event.kind === 'tab' || event.kind === 'header_space');

  return allowedReorder;
}

export function vetoInvalidWorkbenchActivityDrop(
  event: WorkbenchActivityDropEvent,
): void {
  if (!shouldAllowWorkbenchActivityDrop(event)) event.preventDefault();
}

export function canMoveWorkbenchPanel(
  metadata: WorkbenchPanelMetadata,
  targetGroupId: string,
  targetPosition?: WorkbenchMoveTargetPosition,
): boolean {
  if (isWorkbenchActivityMetadata(metadata)) {
    return targetGroupId === WORKBENCH_ACTIVITY_GROUP_ID;
  }
  if (isWorkbenchPersistentViewMetadata(metadata)) {
    return targetPosition === 'right' || targetGroupId === WORKBENCH_EDGE_GROUP_IDS.right;
  }
  return targetGroupId !== WORKBENCH_ACTIVITY_GROUP_ID;
}

export function canSplitWorkbenchPanel(
  metadata: WorkbenchPanelMetadata,
  referenceGroupId: string,
): boolean {
  return !isWorkbenchActivityMetadata(metadata)
    && !isWorkbenchPersistentViewMetadata(metadata)
    && referenceGroupId !== WORKBENCH_ACTIVITY_GROUP_ID;
}

export function canRemoveWorkbenchPanel(metadata: WorkbenchPanelMetadata): boolean {
  return !isWorkbenchActivityMetadata(metadata)
    && !isWorkbenchPersistentViewMetadata(metadata);
}
