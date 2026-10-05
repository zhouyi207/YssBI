import type { IJsonModel, IJsonRowNode, IJsonTabSetNode } from "flexlayout-react";
import { WORKBENCH_ACTIVITY_VIEW_IDS } from "./workbenchPanelModel";
import type {
  WorkbenchLayoutTransaction,
  WorkbenchPanelInfo,
  WorkbenchEdgePosition,
} from "./workbenchTypes";
export const WORKBENCH_ACTIVITY_GROUP_ID = "border_left";
export const WORKBENCH_CONVERSATION_GROUP_ID = "assistant-conversations";
export const WORKBENCH_WORKSPACE_LAYOUT_ID = "workbench-workspace";
export const WORKBENCH_WORKSPACE_GROUP_ID = "workbench-workspace-host";
export const WORKBENCH_WORKSPACE_PANEL_ID = "workbench-workspace-content";
export const WORKBENCH_WORKSPACE_GROUP_ATTRIBUTES = {
  id: WORKBENCH_WORKSPACE_GROUP_ID,
  enableClose: false,
  enableDeleteWhenEmpty: false,
  enableTabStrip: false,
  enableDrag: false,
  enableDrop: false,
  enableDivide: false,
  enableMaximize: false,
} as const;
export const WORKBENCH_CONVERSATION_GROUP_ATTRIBUTES = {
  id: WORKBENCH_CONVERSATION_GROUP_ID,
  minWidth: 320,
  enableTabStrip: false,
  enableDrag: false,
  enableDivide: false,
  enableMaximize: false,
} as const;
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
  assistant: "left",
  result: "right",
  reference: "right",
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
      tabEnableFloatIcon: false,
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
      children: [
        {
          type: "tabset",
          ...WORKBENCH_WORKSPACE_GROUP_ATTRIBUTES,
          children: [
            {
              type: "tab",
              id: WORKBENCH_WORKSPACE_PANEL_ID,
              name: "",
              subLayoutId: WORKBENCH_WORKSPACE_LAYOUT_ID,
              contentClassName: "workbench-workspace-content",
              enableClose: false,
              enableDrag: false,
            },
          ],
        },
      ],
    },
    subLayouts: {
      [WORKBENCH_WORKSPACE_LAYOUT_ID]: { type: "tab", layout: { type: "row", children: [] } },
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
    if (bucket === "grid") visit(layout.subLayouts![WORKBENCH_WORKSPACE_LAYOUT_ID].layout);
    else
      for (const tab of layout.borders?.find((border) => border.location === bucket)?.children ??
        []) {
        if (tab.id) ids.push(tab.id);
      }
  }
  const live = new Set(livePanelIds);
  return [...new Set([...ids, ...livePanelIds])].filter((id) => live.has(id));
}

export function readSerializedEdge(layout: IJsonModel, position: WorkbenchEdgePosition) {
  const border = layout.borders?.find((node) => node.location === position);
  if (!border) return undefined;
  return {
    size: border.size ?? WORKBENCH_EDGE_SIZES[position as keyof typeof WORKBENCH_EDGE_SIZES] ?? 200,
    collapsed: (border.selected ?? -1) < 0,
    activePanelId: border.children?.[border.selected ?? -1]?.id,
  };
}

const DETAILS_VIEW_REQUEST = {
  viewId: "details",
  title: "Details",
} as const;

function persistedRightSidebarSize(transaction: WorkbenchLayoutTransaction): number {
  const right = readSerializedEdge(transaction.serialize(), "right");
  return typeof right?.size === "number" ? right.size : WORKBENCH_EDGE_SIZES.right;
}

function configurePermanentDetailsSidebar(
  transaction: WorkbenchLayoutTransaction,
  details: WorkbenchPanelInfo,
): void {
  const right = transaction.configureEdge({
    position: "right",
    size: persistedRightSidebarSize(transaction),
    collapsed: false,
  });
  transaction.move({
    panelInstanceId: details.panelInstanceId,
    groupId: right.groupId,
    index: 0,
    activate: false,
  });
}

function ensurePermanentDetailsSidebar(transaction: WorkbenchLayoutTransaction): void {
  const activePanelInstanceId = transaction.getActivePanel()?.panelInstanceId;
  const right = readSerializedEdge(transaction.serialize(), "right");
  const details = transaction.ensureView(DETAILS_VIEW_REQUEST);
  configurePermanentDetailsSidebar(transaction, details);
  if (right) {
    if (right.activePanelId && transaction.getPanel(right.activePanelId)) {
      transaction.activate(right.activePanelId);
    }
    transaction.configureEdge({ position: "right", size: right.size, collapsed: right.collapsed });
  }
  if (activePanelInstanceId && transaction.getPanel(activePanelInstanceId)) {
    transaction.activate(activePanelInstanceId);
  }
}

export function installDefaultRootLayout(transaction: WorkbenchLayoutTransaction): void {
  transaction.ensureCentralGroup();
  const activityPanels = WORKBENCH_ACTIVITY_DEFAULT_ORDER.map((viewId) =>
    transaction.ensureView({ viewId, title: viewId[0].toUpperCase() + viewId.slice(1) }),
  );
  const details = transaction.ensureView(DETAILS_VIEW_REQUEST);
  const logs = transaction.ensureView({ viewId: "logs", title: "Logs" });
  const output = transaction.ensureView({ viewId: "output", title: "Output" });
  const problems = transaction.ensureView({ viewId: "problems", title: "Problems" });
  const left = transaction.configureEdge({
    position: "left",
    size: WORKBENCH_EDGE_SIZES.left,
    collapsed: false,
  });
  configurePermanentDetailsSidebar(transaction, details);
  const bottom = transaction.configureEdge({
    position: "bottom",
    size: WORKBENCH_EDGE_SIZES.bottom,
    collapsed: false,
  });
  activityPanels.forEach((panel, index) => {
    transaction.move({
      panelInstanceId: panel.panelInstanceId,
      groupId: left.groupId,
      index,
    });
  });
  for (const [index, viewId] of WORKBENCH_BOTTOM_DEFAULT_ORDER.entries()) {
    transaction.move({
      panelInstanceId: { logs, output, problems }[viewId].panelInstanceId,
      groupId: bottom.groupId,
      index,
    });
  }
  const project = activityPanels.find(
    (panel) => panel.metadata.role === "view" && panel.metadata.viewId === "project",
  );
  if (project) transaction.activate(project.panelInstanceId);
  transaction.configureEdge({
    position: "bottom",
    size: WORKBENCH_EDGE_SIZES.bottom,
    collapsed: true,
  });
}

export function ensureRestoredRootPanels(transaction: WorkbenchLayoutTransaction): void {
  ensurePermanentDetailsSidebar(transaction);
  const panels = transaction.listPanels();
  const missingActivities = WORKBENCH_ACTIVITY_DEFAULT_ORDER.filter(
    (viewId) =>
      !panels.some((panel) => panel.metadata.role === "view" && panel.metadata.viewId === viewId),
  );
  if (missingActivities.length > 0) {
    const active = transaction.getActivePanel();
    const left = readSerializedEdge(transaction.serialize(), "left");
    const leftGroup = left ? { activeView: left.activePanelId } : undefined;
    for (const viewId of missingActivities) {
      transaction.ensureView({ viewId, title: viewId[0].toUpperCase() + viewId.slice(1) });
    }
    if (leftGroup?.activeView) transaction.activate(leftGroup.activeView);
    if (active) transaction.activate(active.panelInstanceId);
    if (left)
      transaction.configureEdge({
        position: "left",
        size: left.size,
        collapsed: left.collapsed ?? false,
      });
  }
}
