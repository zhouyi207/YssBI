import type {
  DockviewWillDropEvent,
  DockviewWillShowOverlayLocationEvent,
  IDockviewPanel,
} from 'dockview-react';

import {
  isWorkbenchActivityMetadata,
  isWorkbenchPanelMetadata,
  type WorkbenchPanelMetadata,
} from './workbenchPanelModel';
import { WORKBENCH_ACTIVITY_GROUP_ID } from './workbenchDockviewDefaults';

export { WORKBENCH_ACTIVITY_GROUP_ID } from './workbenchDockviewDefaults';

export type WorkbenchActivityDropEvent =
  | DockviewWillDropEvent
  | DockviewWillShowOverlayLocationEvent;

type ActivityPanelLike = Pick<IDockviewPanel, 'params'>;

function activityMetadata(panel: ActivityPanelLike | undefined): WorkbenchPanelMetadata | undefined {
  const metadata = panel?.params && typeof panel.params === 'object'
    ? (panel.params as { metadata?: unknown }).metadata
    : undefined;
  return isWorkbenchPanelMetadata(metadata) && isWorkbenchActivityMetadata(metadata)
    ? metadata
    : undefined;
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

function sourcePanel(
  event: Pick<WorkbenchActivityDropEvent, 'api' | 'getData'>,
): IDockviewPanel | undefined {
  const transfer = event.getData();
  if (!transfer || transfer.viewId !== event.api.id || transfer.panelId === null) return undefined;
  return event.api.getPanel(transfer.panelId);
}

export function shouldAllowWorkbenchActivityDrop(
  event: WorkbenchActivityDropEvent,
): boolean {
  const transfer = event.getData();
  const source = sourcePanel(event);
  const sourceIsActivity = isWorkbenchActivityPanel(source)
    || transfer?.groupId === WORKBENCH_ACTIVITY_GROUP_ID;
  const targetIsActivity = targetGroupId(event) === WORKBENCH_ACTIVITY_GROUP_ID
    || isWorkbenchActivityPanel(event.panel);
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
): boolean {
  if (isWorkbenchActivityMetadata(metadata)) {
    return targetGroupId === WORKBENCH_ACTIVITY_GROUP_ID;
  }
  return targetGroupId !== WORKBENCH_ACTIVITY_GROUP_ID;
}

export function canSplitWorkbenchPanel(
  metadata: WorkbenchPanelMetadata,
  referenceGroupId: string,
): boolean {
  return !isWorkbenchActivityMetadata(metadata)
    && referenceGroupId !== WORKBENCH_ACTIVITY_GROUP_ID;
}

export function canRemoveWorkbenchPanel(metadata: WorkbenchPanelMetadata): boolean {
  return !isWorkbenchActivityMetadata(metadata);
}
