import type { ContextMenuSection } from '@/shared/ui/contextMenu';

import { isGraphResourceDirty } from '@/features/core/resource';

import { isPreviewLayoutTab, layoutTabResourceRef } from '@/features/core/layout/layoutTabModel';
import { isStickyLayoutTab } from '@/features/core/layout/tabBarOrder';

import { useEditorTabStore } from '@/features/core/layout/editorTabStore';

import type { LayoutTab } from '@/shared/types/ui';

import type { TFunction } from 'i18next';

import {

  closeAllTabsInGroup,

  closeOtherTabs,

  closeSavedTabsInGroup,

  closeTab,

  pinTab,

  setTabSticky,

} from './tabCommands';



export interface TabContextMenuActions {

  revealInSidebar?: (tab: LayoutTab) => void;

}



function groupHasSavedTabs(groupId: string): boolean {

  const tabs = useEditorTabStore.getState().resolveGroupTabs(groupId);

  return tabs.some((item) => {

    const ref = layoutTabResourceRef(item);

    if (!ref || (ref.kind !== 'event' && ref.kind !== 'function' && ref.kind !== 'worksheet')) {

      return false;

    }

    return !isGraphResourceDirty(ref.id, ref.kind);

  });

}



export function buildTabContextMenuSections(

  groupId: string,

  tab: LayoutTab,

  t: TFunction,

  actions?: TabContextMenuActions,

): ContextMenuSection[] {

  const canReveal =

    Boolean(actions?.revealInSidebar) &&

    (tab.type === 'event' || tab.type === 'function');



  const sections: ContextMenuSection[] = [];



  if (isPreviewLayoutTab(tab)) {

    sections.push({

      items: [

        {

          id: 'keep-open',

          label: t('tabBar.contextMenu.keepOpen'),

          onClick: () => pinTab(groupId, tab.id),

        },

      ],

    });

  }



  if (tab.type === 'event' || tab.type === 'function' || tab.type === 'worksheet') {

    sections.push({

      items: [

        {

          id: 'toggle-sticky',

          label: isStickyLayoutTab(tab)

            ? t('tabBar.contextMenu.unstickTab')

            : t('tabBar.contextMenu.stickTab'),

          onClick: () => void setTabSticky(groupId, tab.id, !tab.sticky),

        },

      ],

    });

  }



  sections.push({

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

  });



  if (canReveal) {

    sections.push({

      items: [

        {

          id: 'reveal',

          label: t('tabBar.contextMenu.revealInSidebar'),

          onClick: () => actions?.revealInSidebar?.(tab),

        },

      ],

    });

  }



  return sections;

}


