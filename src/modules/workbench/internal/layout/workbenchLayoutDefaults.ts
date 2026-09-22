import type { IJsonModel, IJsonRowNode, IJsonTabSetNode } from "flexlayout-react";
import { WORKBENCH_ACTIVITY_VIEW_IDS } from "./workbenchPanelModel";
export const WORKBENCH_ACTIVITY_GROUP_ID = "border_left";
export const WORKBENCH_EDGE_GROUP_IDS = {
  left: "border_left",
  right: "border_right",
  bottom: "border_bottom",
} as const;
export const WORKBENCH_EDGE_SIZES = { left: 280, right: 330, bottom: 240 } as const;
export const WORKBENCH_HOME_LOCATION = {
  project: "left",
  nodes: "left",
  commands: "left",
  plugins: "left",
  details: "right",
  assistant: "right",
  result: "right",
  logs: "bottom",
  output: "bottom",
  problems: "bottom",
} as const;
export const WORKBENCH_ACTIVITY_DEFAULT_ORDER = WORKBENCH_ACTIVITY_VIEW_IDS;
export const WORKBENCH_BOTTOM_DEFAULT_ORDER = ["problems", "output", "logs"] as const;
export const WORKBENCH_RESET_BUCKET_ORDER = ["left", "top", "grid", "right", "bottom"] as const;
export function createEmptyWorkbenchLayout(): IJsonModel {
  return {
    global: {
      tabEnableFloat: false,
      tabEnablePopout: false,
      tabEnableRename: false,
      tabEnablePin: false,
      tabEnableRenderOnDemand: true,
      tabSetEnableDeleteWhenEmpty: true,
      // FlexLayout requires both flags to remove a tabset after its last tab leaves.
      tabSetEnableClose: true,
      tabSetEnableCloseButton: false,
      tabSetEnableActiveIcon: false,
      tabSetMinWidth: 160,
      tabSetMinHeight: 100,
      borderEnableAutoHide: true,
      borderMinSize: 180,
      tabSetEnableTabGroups: false,
    },
    borders: [
      {
        type: "border",
        location: "left",
        size: WORKBENCH_EDGE_SIZES.left,
        selected: -1,
        children: [],
      },
      {
        type: "border",
        location: "right",
        size: WORKBENCH_EDGE_SIZES.right,
        selected: -1,
        children: [],
      },
      {
        type: "border",
        location: "bottom",
        // The native bottom strip also hosts status information when its panels are closed.
        enableAutoHide: false,
        size: WORKBENCH_EDGE_SIZES.bottom,
        selected: -1,
        children: [],
      },
      { type: "border", location: "top", show: false, selected: -1, children: [] },
    ],
    layout: {
      type: "row",
      children: [],
    },
  };
}
export function orderWorkbenchPanelIdsForReset(
  layout: IJsonModel,
  livePanelIds: readonly string[],
): readonly string[] {
  const ids: string[] = [];
  const visit = (node: IJsonRowNode | IJsonTabSetNode): void => {
    for (const child of node.children ?? []) {
      if (child.type === "tab") {
        if (child.id) ids.push(child.id);
      } else if (child.type === "row" || child.type === "tabset") visit(child);
    }
  };
  for (const bucket of WORKBENCH_RESET_BUCKET_ORDER) {
    if (bucket === "grid") visit(layout.layout);
    else
      for (const tab of layout.borders?.find((border) => border.location === bucket)?.children ??
        []) {
        if (tab.id) ids.push(tab.id);
      }
  }
  const live = new Set(livePanelIds);
  return [...new Set([...ids, ...livePanelIds])].filter((id) => live.has(id));
}
