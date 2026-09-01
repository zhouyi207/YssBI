import { createElement } from "react";
import type { TFunction } from "i18next";
import { VscCheckAll, VscClearAll, VscClose, VscCloseAll } from "react-icons/vsc";
import { isResourceDocumentDirty } from "@/features/core/resource";
import type { ActionMenuSection } from "@/shared/ui/actionMenu";
import { workbenchDockviewRead } from "@/modules/workbench/public";
import {
  requestCloseAllEditorPanelsInGroup,
  requestCloseEditorPanel,
  requestCloseOtherEditorPanels,
  requestCloseSavedEditorPanelsInGroup,
} from "./editorPanelCloseCommands";

export interface EditorPanelTabMenuTarget {
  readonly panelInstanceId: string;
  readonly groupId: string;
}

function groupHasSavedEditorPanels(groupId: string): boolean {
  return workbenchDockviewRead.listEditorPanelsInGroup(groupId).some(
    (panel) =>
      !isResourceDocumentDirty({
        id: panel.metadata.resourceRef,
        kind: panel.metadata.resourceKind,
      }),
  );
}

export function buildEditorPanelTabMenu(
  target: EditorPanelTabMenuTarget,
  t: TFunction,
): ActionMenuSection[] {
  const { groupId, panelInstanceId } = target;
  const sections: ActionMenuSection[] = [
    {
      items: [
        {
          id: "close",
          label: t("tabBar.contextMenu.close"),
          icon: createElement(VscClose, { size: 12 }),
          onClick: () => void requestCloseEditorPanel(panelInstanceId),
        },
      ],
    },
    {
      items: [
        {
          id: "close-others",
          label: t("tabBar.contextMenu.closeOthers"),
          icon: createElement(VscCloseAll, { size: 12 }),
          onClick: () => void requestCloseOtherEditorPanels(groupId, panelInstanceId),
        },
        {
          id: "close-saved",
          label: t("tabBar.contextMenu.closeSaved"),
          icon: createElement(VscCheckAll, { size: 12 }),
          disabled: !groupHasSavedEditorPanels(groupId),
          onClick: () => void requestCloseSavedEditorPanelsInGroup(groupId),
        },
        {
          id: "close-all",
          label: t("tabBar.contextMenu.closeAll"),
          icon: createElement(VscClearAll, { size: 12 }),
          onClick: () => void requestCloseAllEditorPanelsInGroup(groupId),
        },
      ],
    },
  ];

  return sections;
}
