import type { TFunction } from 'i18next';

export type MenubarMenuItem = {
  label: string;
  shortcut?: string;
  onClick?: () => void;
  type?: 'item' | 'checkbox' | 'separator';
  checked?: boolean;
};

export interface MenubarViewState {
  readonly resourcesOpen: boolean;
  readonly detailsOpen: boolean;
  readonly detailsContextValid: boolean;
  readonly inspectOpen: boolean;
  readonly inspectContextValid: boolean;
  readonly logsOpen: boolean;
  readonly outputOpen: boolean;
  readonly bottomCollapsed: boolean;
}

export interface MenubarViewMenuActions {
  readonly toggleResources: () => void;
  readonly toggleDetails: () => void;
  readonly toggleInspect: () => void;
  readonly toggleLogs: () => void;
  readonly toggleOutput: () => void;
  readonly resetLayout: () => void;
}

/** View menu projected from live root Dockview panels, never mirrored visibility state. */
export function buildViewMenuItems(
  t: TFunction,
  state: MenubarViewState,
  actions: MenubarViewMenuActions,
): MenubarMenuItem[] {
  return [
    {
      label: t('panel.resources'),
      type: 'checkbox',
      checked: state.resourcesOpen,
      onClick: actions.toggleResources,
    },
    {
      label: t('panel.details'),
      type: 'checkbox',
      checked: state.detailsOpen,
      onClick: state.detailsOpen || state.detailsContextValid
        ? actions.toggleDetails
        : undefined,
    },
    {
      label: t('panel.inspect'),
      type: 'checkbox',
      checked: state.inspectOpen,
      onClick: state.inspectOpen || state.inspectContextValid
        ? actions.toggleInspect
        : undefined,
    },
    {
      label: t('panel.logs'),
      type: 'checkbox',
      checked: state.logsOpen,
      onClick: actions.toggleLogs,
    },
    {
      label: t('panel.output'),
      type: 'checkbox',
      checked: state.outputOpen,
      onClick: actions.toggleOutput,
    },
    { label: '-', type: 'separator' },
    {
      label: t('menubar.resetLayout'),
      onClick: actions.resetLayout,
    },
  ];
}
