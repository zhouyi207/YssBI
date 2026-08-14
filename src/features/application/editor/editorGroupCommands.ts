import type { EditorSplitEdge } from '@/features/core/layout/editorSplitLayout';
import { editorDockviewPort, type SplitDirection } from '@/features/core/dockview';
import { getActiveLayoutTab } from '@/features/core/layout/layoutTabQueries';
import { switchEditorTab } from './switchEditorTab';
import {
  findDockviewPanel,
  layoutTabFromDockviewPanel,
  listDockviewGroupPanels,
} from './dockviewTabProjection';

let copiedPanelSequence = 0;

function copiedPanelId(sourcePanelId: string): string {
  copiedPanelSequence += 1;
  return `${sourcePanelId}-copy-${copiedPanelSequence}`;
}

function splitDirection(edge: EditorSplitEdge): SplitDirection | null {
  return edge === 'center' ? null : edge;
}

async function activateCreatedEditorGroup(groupId: string | null): Promise<string | null> {
  if (!groupId) return null;
  const activeTab = getActiveLayoutTab(groupId)?.tab;
  if (activeTab) await switchEditorTab(groupId, activeTab);
  return groupId;
}

async function copyPanelToGroup(
  panel: ReturnType<typeof editorDockviewPort.listPanels>[number],
  targetGroupId: string,
  index?: number,
): Promise<string | null> {
  const tab = layoutTabFromDockviewPanel(panel);
  if (!tab || !panel.tab) return null;
  const panelInstanceId = copiedPanelId(panel.panelInstanceId);
  await editorDockviewPort.open({
    panelInstanceId,
    component: panel.component,
    title: panel.title,
    groupId: targetGroupId,
    index,
    tab: panel.tab,
  });
  return panelInstanceId;
}

/** Programmatic counterpart to Dockview's native panel move/order operation. */
export function moveTabsBetweenGroups(
  sourceGroupId: string,
  tabIds: string[],
  targetGroupId: string,
  targetTabIndex?: number,
): void {
  void (async () => {
    let index = targetTabIndex;
    for (const tabId of tabIds) {
      const panel = findDockviewPanel(tabId, sourceGroupId);
      if (!panel) continue;
      await editorDockviewPort.move({
        panelInstanceId: panel.panelInstanceId,
        groupId: targetGroupId,
        index,
      });
      if (index !== undefined) index += 1;
    }
  })();
}

export function moveTabBetweenGroups(
  sourceGroupId: string,
  tabId: string,
  targetGroupId: string,
  targetTabIndex?: number,
): void {
  moveTabsBetweenGroups(sourceGroupId, [tabId], targetGroupId, targetTabIndex);
}

export function copyTabsBetweenGroups(
  sourceGroupId: string,
  tabIds: string[],
  targetGroupId: string,
  targetTabIndex?: number,
): void {
  void (async () => {
    let index = targetTabIndex;
    for (const tabId of tabIds) {
      const panel = findDockviewPanel(tabId, sourceGroupId);
      if (!panel) continue;
      await copyPanelToGroup(panel, targetGroupId, index);
      if (index !== undefined) index += 1;
    }
  })();
}

export function copyTabBetweenGroups(
  sourceGroupId: string,
  tabId: string,
  targetGroupId: string,
  targetTabIndex?: number,
): void {
  copyTabsBetweenGroups(sourceGroupId, [tabId], targetGroupId, targetTabIndex);
}

export async function splitEditorWithTab(
  sourceGroupId: string,
  tabId: string,
  targetGroupId: string,
  edge: EditorSplitEdge,
  options?: { copy?: boolean },
): Promise<string | null> {
  const sourcePanel = findDockviewPanel(tabId, sourceGroupId);
  if (!sourcePanel) return null;

  let panelInstanceId = sourcePanel.panelInstanceId;
  if (options?.copy) {
    const copiedId = await copyPanelToGroup(sourcePanel, targetGroupId);
    if (!copiedId) return null;
    panelInstanceId = copiedId;
  }

  const direction = splitDirection(edge);
  if (!direction) return null;
  const split = await editorDockviewPort.split({
    panelInstanceId,
    referenceGroupId: targetGroupId,
    direction,
  });
  if (!split) return null;
  const createdGroupId = editorDockviewPort
    .listPanels()
    .find((panel) => panel.panelInstanceId === panelInstanceId)?.groupId ?? null;
  return activateCreatedEditorGroup(createdGroupId);
}

export function mergeEditorGroupInto(
  sourceGroupId: string,
  targetGroupId: string,
  insertIndex?: number,
): void {
  const tabIds = listDockviewGroupPanels(sourceGroupId)
    .map(layoutTabFromDockviewPanel)
    .filter((tab) => tab !== null)
    .map((tab) => tab.id);
  moveTabsBetweenGroups(sourceGroupId, tabIds, targetGroupId, insertIndex);
}

export function copyEditorGroupInto(
  sourceGroupId: string,
  targetGroupId: string,
  insertIndex?: number,
): void {
  const tabIds = listDockviewGroupPanels(sourceGroupId)
    .map(layoutTabFromDockviewPanel)
    .filter((tab) => tab !== null)
    .map((tab) => tab.id);
  copyTabsBetweenGroups(sourceGroupId, tabIds, targetGroupId, insertIndex);
}

export async function splitEditorGroupWithGroup(
  sourceGroupId: string,
  targetGroupId: string,
  edge: EditorSplitEdge,
): Promise<string | null> {
  const panels = listDockviewGroupPanels(sourceGroupId);
  const first = panels[0];
  if (!first) return null;
  const direction = splitDirection(edge);
  if (!direction) return null;
  const split = await editorDockviewPort.split({
    panelInstanceId: first.panelInstanceId,
    referenceGroupId: targetGroupId,
    direction,
  });
  if (!split) return null;
  const createdGroupId = findDockviewPanel(first.tab?.resourceRef ?? '', undefined)?.groupId;
  if (!createdGroupId) return null;
  for (const panel of panels.slice(1)) {
    await editorDockviewPort.move({ panelInstanceId: panel.panelInstanceId, groupId: createdGroupId });
  }
  return activateCreatedEditorGroup(createdGroupId);
}

export async function copyEditorGroupWithSplit(
  sourceGroupId: string,
  targetGroupId: string,
  edge: EditorSplitEdge,
): Promise<string | null> {
  const panels = listDockviewGroupPanels(sourceGroupId);
  const first = panels[0];
  if (!first) return null;
  const copiedId = await copyPanelToGroup(first, targetGroupId);
  if (!copiedId) return null;
  const direction = splitDirection(edge);
  if (!direction) return null;
  const split = await editorDockviewPort.split({
    panelInstanceId: copiedId,
    referenceGroupId: targetGroupId,
    direction,
  });
  if (!split) return null;
  const createdGroupId = editorDockviewPort
    .listPanels()
    .find((panel) => panel.panelInstanceId === copiedId)?.groupId;
  if (!createdGroupId) return null;
  for (const panel of panels.slice(1)) await copyPanelToGroup(panel, createdGroupId);
  return activateCreatedEditorGroup(createdGroupId);
}

export async function splitOrMoveSingleTabGroup(
  sourceGroupId: string,
  tabId: string,
  targetGroupId: string,
  edge: EditorSplitEdge,
): Promise<string | null> {
  return splitEditorWithTab(sourceGroupId, tabId, targetGroupId, edge);
}

/** Split the active Dockview panel. Dockview owns the resulting group topology. */
export async function splitEditorAtEdge(
  groupId: string,
  edge: 'right' | 'bottom',
): Promise<string | null> {
  const activePanelId = editorDockviewPort
    .listGroups()
    .find((group) => group.groupId === groupId)?.activePanelInstanceId;
  if (!activePanelId) return null;
  const split = await editorDockviewPort.split({
    panelInstanceId: activePanelId,
    referenceGroupId: groupId,
    direction: edge,
  });
  if (!split) return null;
  const createdGroupId = editorDockviewPort
    .listPanels()
    .find((panel) => panel.panelInstanceId === activePanelId)?.groupId ?? null;
  return activateCreatedEditorGroup(createdGroupId);
}
