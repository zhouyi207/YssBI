import type { TFunction } from 'i18next';

export type MenubarMenuItem = {
  label: string;
  shortcut?: string;
  onClick?: () => void;
  type?: 'item' | 'separator';
};

export type MenubarShellVisibilityState = {
  isSidebarVisible: boolean;
  isDetailVisible: boolean;
  isLogPanelVisible: boolean;
  zenMode: boolean;
};

export type MenubarShellVisibilityActions = {
  toggleSidebar: () => void;
  toggleDetail: () => void;
  toggleLogPanel: () => void;
  toggleZenMode: () => void;
};

export type MenubarViewMenuActions = MenubarShellVisibilityActions & {
  resetLayout: () => void;
};

/** View → shell visibility + reset chrome layout (VS Code View menu parity). */
export function buildViewMenuItems(
  t: TFunction,
  state: MenubarShellVisibilityState,
  actions: MenubarViewMenuActions,
): MenubarMenuItem[] {
  return [
    {
      label: state.isSidebarVisible ? t('menubar.hidePrimarySideBar') : t('menubar.showPrimarySideBar'),
      shortcut: 'Ctrl+B',
      onClick: actions.toggleSidebar,
    },
    {
      label: state.isDetailVisible ? t('menubar.hideSecondarySideBar') : t('menubar.showSecondarySideBar'),
      shortcut: 'Ctrl+I',
      onClick: actions.toggleDetail,
    },
    {
      label: state.isLogPanelVisible ? t('menubar.hidePanel') : t('menubar.showPanel'),
      shortcut: 'Ctrl+`',
      onClick: actions.toggleLogPanel,
    },
    { label: '-' },
    {
      label: state.zenMode ? t('menubar.exitZenMode') : t('menubar.enterZenMode'),
      shortcut: 'Ctrl+K Z',
      onClick: actions.toggleZenMode,
    },
    { label: '-' },
    {
      label: t('menubar.resetLayout'),
      onClick: actions.resetLayout,
    },
  ];
}
