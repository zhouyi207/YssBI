import type {
  EditorGroupSnapshot,
  LayoutNode,
  LayoutTab,
  LayoutTabComponent,
  LayoutTabType,
} from '@/shared/types';

/** hydrate 入站：历史 Tab 可能缺 type / component */
export type LayoutTabInput = Omit<LayoutTab, 'type' | 'component'> & {
  type?: LayoutTabType;
  component?: LayoutTabComponent | string;
};

/** 从持久化/旧数据规范化 Tab（补全缺失的 type / component） */
export function normalizeLayoutTab(tab: LayoutTabInput): LayoutTab {
  const type: LayoutTabType =
    tab.type ??
    (tab.component === 'WorksheetEditor' ? 'worksheet' : 'event');
  const component: LayoutTabComponent =
    type === 'worksheet' ? 'WorksheetEditor' : 'GraphEditor';
  return { ...tab, type, component };
}

export function normalizeLayoutTabs(tabs: readonly LayoutTabInput[]): LayoutTab[] {
  return tabs.map(normalizeLayoutTab);
}

export function buildGraphLayoutTab(
  id: string,
  title: string,
  type: 'event' | 'function',
): LayoutTab {
  return { id, title, type, component: 'GraphEditor' };
}

export function buildWorksheetLayoutTab(id: string, title: string): LayoutTab {
  return { id, title, type: 'worksheet', component: 'WorksheetEditor' };
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

/** splitNode 复用：按当前 Tab 选择编辑器组件 */
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
