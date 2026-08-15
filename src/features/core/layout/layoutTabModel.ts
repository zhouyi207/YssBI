import type {
  LayoutTab,
  LayoutTabComponent,
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
  type: GraphResourceKind,
  options?: { pinned?: boolean },
): LayoutTab {
  if (!isValidGraphResourceTabId(path, type)) {
    throw new Error(`Invalid graph tab id for ${type}: ${path}`);
  }
  const pinned = options?.pinned !== false;
  return { id: path, type, component: 'GraphEditor', pinned: pinned ? true : false };
}

export function buildWorksheetLayoutTab(
  worksheetPath: string,
  options?: { pinned?: boolean },
): LayoutTab {
  const pinned = options?.pinned !== false;
  return {
    id: worksheetPath,
    type: 'worksheet',
    component: 'WorksheetEditor',
    pinned: pinned ? true : false,
  };
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
