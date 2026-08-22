import { createElement } from 'react';
import type { TFunction } from 'i18next';
import { VscCheckAll, VscClearAll, VscClose, VscCloseAll } from 'react-icons/vsc';
import { isGraphResourceDirty } from '@/features/core/resource';
import type { ActionMenuSection } from '@/shared/ui/actionMenu';
import type { LayoutTab } from '@/shared/types/ui';
import {
  closeAllTabsInGroup,
  closeOtherTabs,
  closeSavedTabsInGroup,
  closeTab,
} from './tabCommands';
import { listDockviewGroupTabs } from './dockviewTabProjection';

function groupHasSavedTabs(groupId: string): boolean {
  return listDockviewGroupTabs(groupId).some((tab) =>
    (tab.type === 'event' || tab.type === 'function' || tab.type === 'worksheet')
      && !isGraphResourceDirty(tab.id, tab.type));
}

export function buildTabContextMenuSections(
  groupId: string,
  tab: LayoutTab,
  t: TFunction,
): ActionMenuSection[] {
  const sections: ActionMenuSection[] = [
    {
      items: [{
        id: 'close',
        label: t('tabBar.contextMenu.close'),
        icon: createElement(VscClose, { size: 12 }),
        onClick: () => void closeTab(groupId, tab.id),
      }],
    },
    {
      items: [
        {
          id: 'close-others',
          label: t('tabBar.contextMenu.closeOthers'),
          icon: createElement(VscCloseAll, { size: 12 }),
          onClick: () => void closeOtherTabs(groupId, tab.id),
        },
        {
          id: 'close-saved',
          label: t('tabBar.contextMenu.closeSaved'),
          icon: createElement(VscCheckAll, { size: 12 }),
          disabled: !groupHasSavedTabs(groupId),
          onClick: () => void closeSavedTabsInGroup(groupId),
        },
        {
          id: 'close-all',
          label: t('tabBar.contextMenu.closeAll'),
          icon: createElement(VscClearAll, { size: 12 }),
          onClick: () => void closeAllTabsInGroup(groupId),
        },
      ],
    },
  ];

  return sections;
}
