import type { TFunction } from "i18next";

export type MenubarMenuItem = {
  label: string;
  shortcut?: string;
  onClick?: () => void;
  type?: "item" | "checkbox" | "separator";
  checked?: boolean;
};

export interface MenubarViewState {
  readonly activityGroupOpen: boolean;
  readonly assistantOpen: boolean;
}

export interface MenubarViewMenuActions {
  readonly toggleActivityGroup: () => void;
  readonly toggleAssistant: () => void;
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
      label: t("panel.primarySideBar"),
      type: "checkbox",
      checked: state.activityGroupOpen,
      onClick: actions.toggleActivityGroup,
    },
    {
      label: t("panel.assistant"),
      type: "checkbox",
      checked: state.assistantOpen,
      onClick: actions.toggleAssistant,
    },
    { label: "-", type: "separator" },
    {
      label: t("menubar.resetLayout"),
      onClick: actions.resetLayout,
    },
  ];
}
