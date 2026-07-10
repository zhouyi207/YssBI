import type {
  EditorGroupSnapshot,
  LayoutNode,
  LayoutTab,
  LayoutTabComponent,
  LayoutTabType,
} from '@/shared/types';
import {
  isValidGraphResourceTabId,
  type GraphResourceKind,
} from '@/shared/types/domain/graphResourcePath';
import { resourceRefFromLayoutTab, type ResourceRef } from '@/features/core/resource/resourceTypes';

/** Map a layout tab to its canonical resource reference (null for chrome-only tabs). */
export function layoutTabResourceRef(tab: LayoutTab): ResourceRef | null {
  return resourceRefFromLayoutTab(tab);
}

/** hydrate 入站：历史 Tab 可能缺 type / component，或携带已废弃的 title 快照 */
export type LayoutTabInput = Omit<LayoutTab, 'type' | 'component'> & {
  title?: string;
  type?: LayoutTabType;
  component?: LayoutTabComponent | string;
};

/** 从持久化/旧数据规范化 Tab（补全缺失的 type / component，剥离 title） */
export function normalizeLayoutTab(tab: LayoutTabInput): LayoutTab {
  const { title: _title, ...rest } = tab;
  const type: LayoutTabType =
    tab.type ??
    (tab.component === 'WorksheetEditor' ? 'worksheet' : 'event');
  const component: LayoutTabComponent =
    type === 'worksheet' ? 'WorksheetEditor' : 'GraphEditor';
  return { ...rest, type, component };
}

export function normalizeLayoutTabs(tabs: readonly LayoutTabInput[]): LayoutTab[] {
  return tabs.map(normalizeLayoutTab);
}

/** Preview tabs use `pinned: false`; omitted or `true` means pinned. */
export function isPreviewLayoutTab(tab: LayoutTab | null | undefined): boolean {
  return tab?.pinned === false;
}

export function applyTabPinState(tab: LayoutTab, pinned: boolean): LayoutTab {
  return pinned ? { ...tab, pinned: true } : { ...tab, pinned: false };
}

export function findPreviewTabInTabs(tabs: readonly LayoutTab[] | undefined): LayoutTab | undefined {
  return tabs?.find((tab) => tab.pinned === false);
}

export function buildGraphLayoutTab(
  path: string,
  _title: string,
  type: GraphResourceKind,
  options?: { pinned?: boolean },
): LayoutTab {
  if (!isValidGraphResourceTabId(path, type)) {
    throw new Error(`Invalid graph tab id for ${type}: ${path}`);
  }
  const pinned = options?.pinned !== false;
  return { id: path, type, component: 'GraphEditor', pinned: pinned ? true : false };
}

export function buildWorksheetLayoutTab(id: string, options?: { pinned?: boolean }): LayoutTab {
  const pinned = options?.pinned !== false;
  return { id, type: 'worksheet', component: 'WorksheetEditor', pinned: pinned ? true : false };
}

export function isGraphLayoutTab(
  tab: LayoutTab | null | undefined,
): tab is LayoutTab & { type: 'event' | 'function'; component: 'GraphEditor' } {
  return tab?.type === 'event' || tab?.type === 'function';
}

export function isWorksheetLayoutTab(
  tab: LayoutTab | null | undefined,
): tab is LayoutTab & { type: 'worksheet'; component: 'WorksheetEditor' } {
  return tab?.type === 'worksheet';
}

/** Editor group split: choose the component for the current tab. */
export function splitComponentForTab(tab: LayoutTab | null | undefined): LayoutTabComponent {
  return tab?.component ?? 'GraphEditor';
}

export function readEditorGroupSnapshot(node: LayoutNode): EditorGroupSnapshot | null {
  if (node.type !== 'component' || !node.data?.tabs) return null;
  return {
    id: node.id,
    tabs: normalizeLayoutTabs(node.data.tabs),
    activeTabId: node.data.activeTabId ?? null,
    selectedNodeIds: node.data.params?.selectedNodeIds ?? [],
  };
}
