import type { LayoutTab } from '@/shared/types/ui';

/** VS Code sticky tabs — pinned to the leading edge of the tab strip for display. */
export function orderTabsForTabBar(tabs: readonly LayoutTab[]): LayoutTab[] {
  const sticky: LayoutTab[] = [];
  const normal: LayoutTab[] = [];
  for (const tab of tabs) {
    if (tab.sticky) sticky.push(tab);
    else normal.push(tab);
  }
  return [...sticky, ...normal];
}

export function isStickyLayoutTab(tab: LayoutTab | null | undefined): boolean {
  return tab?.sticky === true;
}
