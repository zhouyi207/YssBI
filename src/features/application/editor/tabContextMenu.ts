import type { TFunction } from 'i18next';
import { isGraphResourceDirty } from '@/features/core/resource';
import type { ContextMenuSection } from '@/shared/ui/contextMenu';
import type { LayoutTab } from '@/shared/types/ui';
import {
  closeAllTabsInGroup,
  closeOtherTabs,
  closeSavedTabsInGroup,
  closeTab,
} from './tabCommands';
import { listDockviewGroupTabs } from './dockviewTabProjection';

export interface TabContextMenuActions {
  revealInSidebar?: (tab: LayoutTab) => void;
}

function groupHasSavedTabs(groupId: string): boolean {
  return listDockviewGroupTabs(groupId).some((tab) =>
    (tab.type === 'event' || tab.type === 'function' || tab.type === 'worksheet')
      && !isGraphResourceDirty(tab.id, tab.type));
}

export function buildTabContextMenuSections(
  groupId: string,
  tab: LayoutTab,
  t: TFunction,
  actions?: TabContextMenuActions,
): ContextMenuSection[] {
  const sections: ContextMenuSection[] = [{
    items: [
      {
        id: 'close',
        label: t('tabBar.contextMenu.close'),
        onClick: () => void closeTab(groupId, tab.id),
      },
      {
        id: 'close-others',
        label: t('tabBar.contextMenu.closeOthers'),
        onClick: () => void closeOtherTabs(groupId, tab.id),
      },
      {
        id: 'close-saved',
        label: t('tabBar.contextMenu.closeSaved'),
        disabled: !groupHasSavedTabs(groupId),
        onClick: () => void closeSavedTabsInGroup(groupId),
      },
      {
        id: 'close-all',
        label: t('tabBar.contextMenu.closeAll'),
        onClick: () => void closeAllTabsInGroup(groupId),
      },
    ],
  }];

  if (actions?.revealInSidebar && (tab.type === 'event' || tab.type === 'function')) {
    sections.push({
      items: [{
        id: 'reveal',
        label: t('tabBar.contextMenu.revealInSidebar'),
        onClick: () => actions.revealInSidebar?.(tab),
      }],
    });
  }

  return sections;
}
