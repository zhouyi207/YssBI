import type { LayoutTab } from '@/shared/types';
import type { DetailFocus } from '@/features/core/editor/detail/types';
import { useEditorStore } from '@/features/core/editor';
import { editorDockviewPort } from '@/features/core/dockview';
import { applyTabPinState, findPreviewTabInTabs } from '@/features/core/layout/layoutTabModel';
import { resolveTabDisplayName } from './resolveTabDisplayName';
import { ensureDetailVisible } from './ensureDetailVisible';

export interface OpenEditorTabOptions {
  targetGroupId?: string;
  insertIndex?: number;
  focusDetail?: DetailFocus;
  /** `false` opens in the preview slot. Default: pinned. */
  pinned?: boolean;
}

let panelSequence = 0;

function createPanelInstanceId(): string {
  panelSequence += 1;
  return `editor-panel-${panelSequence}`;
}

function tabFromPanel(panel: ReturnType<typeof editorDockviewPort.listPanels>[number]): LayoutTab | null {
  const data = panel.tab?.data?.layoutTab;
  if (!data || typeof data !== 'object') return null;
  return data as unknown as LayoutTab;
}

/** Open or activate an editor panel. Dockview owns group topology and placement. */
export function openEditorTab(tab: LayoutTab, options?: OpenEditorTabOptions): void {
  const pinned = options?.pinned !== false;
  const tabToOpen = applyTabPinState(tab, pinned);
  const existing = editorDockviewPort.findPanelsByResource(tab.id)[0];

  if (existing) {
    if (existing.tab) {
      const currentTab = tabFromPanel(existing);
      if (currentTab?.pinned === false && pinned) {
        void editorDockviewPort.updateTab(existing.panelInstanceId, {
          ...existing.tab,
          data: { ...existing.tab.data, layoutTab: { ...currentTab, pinned: true } },
        });
      }
    }
    if (options?.targetGroupId && existing.groupId !== options.targetGroupId) {
      void editorDockviewPort.move({
        panelInstanceId: existing.panelInstanceId,
        groupId: options.targetGroupId,
        index: options.insertIndex,
      });
    } else {
      void editorDockviewPort.activate(existing.panelInstanceId);
    }
  } else {
    if (!pinned) replacePreviewPanel(options?.targetGroupId);
    const title = resolveTabDisplayName(
      tab.type === 'event' || tab.type === 'function' || tab.type === 'worksheet'
        ? { id: tab.id, kind: tab.type }
        : null,
      tab.id,
    );
    void editorDockviewPort.open({
      panelInstanceId: createPanelInstanceId(),
      component: tab.component,
      title,
      groupId: options?.targetGroupId,
      index: options?.insertIndex,
      tab: {
        resourceRef: tab.id,
        kind: tab.type,
        data: { layoutTab: tabToOpen },
      },
    });
  }

  if (options?.focusDetail) useEditorStore.getState().setDetailFocus(options.focusDetail);
  ensureDetailVisible();
}

function replacePreviewPanel(groupId?: string): void {
  const panels = editorDockviewPort
    .listPanels()
    .filter((panel) => !groupId || panel.groupId === groupId);
  const tabs = panels.map(tabFromPanel).filter((tab): tab is LayoutTab => tab !== null);
  const preview = findPreviewTabInTabs(tabs);
  if (!preview) return;
  const panel = panels.find((candidate) => candidate.tab?.resourceRef === preview.id);
  if (panel) void editorDockviewPort.remove(panel.panelInstanceId);
}
