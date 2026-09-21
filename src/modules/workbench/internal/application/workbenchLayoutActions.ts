import { readSerializedEdge } from "../layout/workbenchLayoutOperations";
import i18n from "i18next";

import { logsLayoutControl } from "../layout/logsControl";
import {
  orderWorkbenchPanelIdsForReset,
  WORKBENCH_ACTIVITY_DEFAULT_ORDER,
  WORKBENCH_EDGE_SIZES,
  WORKBENCH_BOTTOM_DEFAULT_ORDER,
} from "../layout/workbenchLayoutDefaults";
import { workbenchLayoutInternal } from "../layout/workbenchLayoutInternal";
import { workbenchLayoutRead, type WorkbenchPanelInfo } from "../layout/workbenchRead";
import { workbenchLayoutControl } from "../layout/workbenchControl";
import {
  isWorkbenchActivityMetadata,
  isWorkbenchPersistentViewMetadata,
  type WorkbenchViewId,
} from "../layout/workbenchPanelModel";
import { closeWorkbenchViewPanel } from "./panelCommands";
import { workbenchLayoutController } from "./workbenchLayoutController";
import { showWorkbenchLayoutError } from "./workbenchLayoutErrorFeedback";
import type { EnsurePluginViewRequest } from "../layout/workbenchTypes";

const VIEW_TITLE_KEYS = {
  project: "activityBar.project",
  nodes: "activityBar.nodes",
  commands: "activityBar.commands",
  plugins: "activityBar.plugins",
  details: "panel.details",
  assistant: "panel.assistant",
  logs: "panel.logs",
  output: "panel.output",
  problems: "panel.problems",
} as const satisfies Record<WorkbenchViewId, string>;

function findWorkbenchView(viewId: WorkbenchViewId): WorkbenchPanelInfo | undefined {
  return workbenchLayoutRead
    .listPanels()
    .find((panel) => panel.metadata.role === "view" && panel.metadata.viewId === viewId);
}

function viewRequest(viewId: WorkbenchViewId) {
  return {
    viewId,
    title: i18n.t(VIEW_TITLE_KEYS[viewId]),
  };
}

async function createAssistantAtDefaultHome(): Promise<WorkbenchPanelInfo | null> {
  let assistantPanelInstanceId: string | undefined;

  await workbenchLayoutInternal.runLayoutTransaction((tx) => {
    const details = tx.ensureView(viewRequest("details"));
    const assistant = tx.ensureView(viewRequest("assistant"));
    const detailsIndex = tx
      .listGroupPanels(details.groupId)
      .findIndex((panel) => panel.panelInstanceId === details.panelInstanceId);

    tx.move({
      panelInstanceId: assistant.panelInstanceId,
      groupId: details.groupId,
      index: detailsIndex < 0 ? 1 : detailsIndex + 1,
      activate: true,
    });
    assistantPanelInstanceId = assistant.panelInstanceId;
  });

  return assistantPanelInstanceId
    ? (workbenchLayoutRead.getPanel(assistantPanelInstanceId) ?? null)
    : null;
}

export async function revealWorkbenchView(
  viewId: WorkbenchViewId,
): Promise<WorkbenchPanelInfo | null> {
  try {
    const existing = findWorkbenchView(viewId);
    if (existing) {
      return (await workbenchLayoutControl.reveal(existing.panelInstanceId)) ? existing : null;
    }
    if (viewId === "assistant") return await createAssistantAtDefaultHome();
    return await workbenchLayoutControl.ensureView(viewRequest(viewId));
  } catch (error) {
    showWorkbenchLayoutError(error);
    return null;
  }
}

export async function toggleWorkbenchView(viewId: WorkbenchViewId): Promise<boolean> {
  const existing = findWorkbenchView(viewId);
  if (
    existing &&
    (isWorkbenchActivityMetadata(existing.metadata) ||
      isWorkbenchPersistentViewMetadata(existing.metadata))
  ) {
    return (await revealWorkbenchView(viewId)) !== null;
  }
  if (existing) return closeWorkbenchViewPanel(existing.panelInstanceId);
  return (await revealWorkbenchView(viewId)) !== null;
}

export async function openPluginWorkbenchView(
  request: EnsurePluginViewRequest,
  reveal = true,
  isCurrent: () => boolean = () => true,
): Promise<void> {
  try {
    await workbenchLayoutInternal.runLayoutTransaction((tx) => {
      if (!isCurrent()) return;
      const existing = tx
        .listPanels()
        .find(
          (panel) =>
            panel.metadata.role === "plugin" &&
            panel.metadata.pluginId === request.pluginId &&
            panel.metadata.viewId === request.viewId,
        );
      if (existing && !reveal) return;
      const active = tx.getActivePanel();
      const left = readSerializedEdge(tx.serialize(), "left");
      const leftGroup = left ? { activeView: left.activePanelId } : undefined;
      tx.ensurePluginView(request);
      if (!reveal) {
        if (leftGroup?.activeView) tx.activate(leftGroup.activeView);
        if (active) tx.activate(active.panelInstanceId);
        if (left)
          tx.configureEdge({
            position: "left",
            size: left.size,
            collapsed: left.collapsed ?? false,
          });
      }
    });
  } catch (error) {
    showWorkbenchLayoutError(error);
  }
}

export async function syncPluginWorkbenchViews(
  installedPluginIds: readonly string[],
  sidebarViews: readonly EnsurePluginViewRequest[],
  isCurrent: () => boolean,
): Promise<void> {
  const installed = new Set(installedPluginIds);
  const removed = await workbenchLayoutInternal.runLayoutTransaction((tx) => {
    if (!isCurrent()) return false;
    const stalePanels = tx
      .listPanels()
      .filter(
        (panel) => panel.metadata.role === "plugin" && !installed.has(panel.metadata.pluginId),
      );
    if (stalePanels.length > 0) tx.removePanels(stalePanels.map((panel) => panel.panelInstanceId));

    const active = tx.getActivePanel();
    const left = readSerializedEdge(tx.serialize(), "left");
    const leftGroup = left ? { activeView: left.activePanelId } : undefined;
    let added = false;
    for (const request of sidebarViews) {
      if (!installed.has(request.pluginId) || request.location !== "sidebar") continue;
      if (
        tx
          .listPanels()
          .some(
            (panel) =>
              panel.metadata.role === "plugin" &&
              panel.metadata.pluginId === request.pluginId &&
              panel.metadata.viewId === request.viewId,
          )
      )
        continue;
      tx.ensurePluginView(request);
      added = true;
    }
    if (added) {
      if (leftGroup?.activeView && tx.getPanel(leftGroup.activeView))
        tx.activate(leftGroup.activeView);
      if (active) tx.activate(active.panelInstanceId);
      if (left)
        tx.configureEdge({
          position: "left",
          size: left.size,
          collapsed: left.collapsed ?? false,
        });
    }
    return stalePanels.length > 0;
  });
  // Persist the live removal through the layout owner, not a parallel storage edit.
  if (removed) await workbenchLayoutController.flushBeforeWindowClose();
}

function activityPanelsInGroup(groupId: string): boolean {
  const panels = workbenchLayoutRead.listGroupPanels(groupId);
  return (
    panels.length >= WORKBENCH_ACTIVITY_DEFAULT_ORDER.length &&
    panels.every((panel) => isWorkbenchActivityMetadata(panel.metadata))
  );
}

async function ensureActivityWorkbenchGroup(): Promise<void> {
  await workbenchLayoutInternal.runLayoutTransaction((tx) => {
    const panels = WORKBENCH_ACTIVITY_DEFAULT_ORDER.map((viewId) =>
      tx.ensureView(viewRequest(viewId)),
    );
    const left = tx.configureEdge({
      position: "left",
      size: WORKBENCH_EDGE_SIZES.left,
      collapsed: false,
    });
    panels.forEach((panel, index) => {
      tx.move({ panelInstanceId: panel.panelInstanceId, groupId: left.groupId, index });
    });
    const project = panels.find(
      (panel) => panel.metadata.role === "view" && panel.metadata.viewId === "project",
    );
    if (project) tx.activate(project.panelInstanceId);
  });
}

export async function toggleActivityWorkbenchGroup(): Promise<void> {
  try {
    const left = workbenchLayoutRead.getEdgeState("left");
    if (left.exists && left.groupId && activityPanelsInGroup(left.groupId)) {
      await workbenchLayoutControl.setEdgeCollapsed("left", left.visible && !left.collapsed);
      return;
    }
    await ensureActivityWorkbenchGroup();
  } catch (error) {
    showWorkbenchLayoutError(error);
  }
}

export async function toggleBottomWorkbenchGroup(): Promise<void> {
  try {
    const bottom = workbenchLayoutRead.getEdgeState("bottom");
    if (
      bottom.exists &&
      bottom.groupId &&
      workbenchLayoutRead.listGroupPanels(bottom.groupId).length > 0
    ) {
      await workbenchLayoutControl.setEdgeCollapsed("bottom", !bottom.collapsed);
      return;
    }

    const logs = findWorkbenchView("logs");
    if (logs) {
      await workbenchLayoutControl.reveal(logs.panelInstanceId);
      return;
    }
  } catch (error) {
    showWorkbenchLayoutError(error);
    return;
  }

  await revealWorkbenchView("logs");
}

export async function resetWorkbenchLayout(): Promise<void> {
  const resetEpoch = workbenchLayoutController.beginLayoutReset();
  try {
    await workbenchLayoutInternal.runLayoutTransaction((tx) => {
      const before = tx.listPanels();
      const beforeById = new Map(before.map((panel) => [panel.panelInstanceId, panel] as const));
      const ordered = orderWorkbenchPanelIdsForReset(
        tx.serialize(),
        before.map((panel) => panel.panelInstanceId),
      ).map((panelId) => beforeById.get(panelId)!);

      const physicallyActive = tx.getActivePanel();
      const editorToRestore =
        (physicallyActive?.metadata.role === "editor" ? physicallyActive : undefined) ??
        ordered.find((panel) => panel.metadata.role === "editor");

      const editors = ordered.filter((panel) => panel.metadata.role === "editor");
      const activityPanels = WORKBENCH_ACTIVITY_DEFAULT_ORDER.map((viewId) =>
        tx.ensureView(viewRequest(viewId)),
      );
      const details = tx.ensureView(viewRequest("details"));
      const assistant = tx.ensureView(viewRequest("assistant"));
      const logs = tx.ensureView(viewRequest("logs"));
      const output = tx.ensureView(viewRequest("output"));
      const problems = tx.ensureView(viewRequest("problems"));
      const left = tx.configureEdge({
        position: "left",
        size: WORKBENCH_EDGE_SIZES.left,
        collapsed: false,
      });
      const right = tx.configureEdge({
        position: "right",
        size: WORKBENCH_EDGE_SIZES.right,
        collapsed: false,
      });
      const bottom = tx.configureEdge({
        position: "bottom",
        size: WORKBENCH_EDGE_SIZES.bottom,
        collapsed: true,
      });
      const centralGroupId =
        tx.listGroups().find((group) => group.location.type === "grid")?.groupId ??
        tx.ensureCentralGroup();

      const firstEditor = editors[0];
      if (firstEditor) {
        tx.move({
          panelInstanceId: firstEditor.panelInstanceId,
          groupId: centralGroupId,
          index: 0,
        });
      }

      activityPanels.forEach((panel, index) => {
        tx.move({
          panelInstanceId: panel.panelInstanceId,
          groupId: left.groupId,
          index,
        });
      });
      tx.move({
        panelInstanceId: details.panelInstanceId,
        groupId: right.groupId,
        index: 0,
        activate: false,
      });
      tx.move({
        panelInstanceId: assistant.panelInstanceId,
        groupId: right.groupId,
        index: 1,
        activate: false,
      });
      for (const [index, viewId] of WORKBENCH_BOTTOM_DEFAULT_ORDER.entries()) {
        tx.move({
          panelInstanceId: { logs, output, problems }[viewId].panelInstanceId,
          groupId: bottom.groupId,
          index,
          activate: false,
        });
      }

      for (const [offset, panel] of editors.slice(1).entries()) {
        tx.move({
          panelInstanceId: panel.panelInstanceId,
          groupId: centralGroupId,
          index: offset + 1,
        });
      }

      const results = ordered.filter((panel) => panel.metadata.role === "result");
      for (const [index, panel] of results.entries()) {
        tx.move({
          panelInstanceId: panel.panelInstanceId,
          groupId: right.groupId,
          index: index + 2,
        });
      }

      const project = activityPanels.find(
        (panel) => panel.metadata.role === "view" && panel.metadata.viewId === "project",
      );
      tx.activate(editorToRestore?.panelInstanceId ?? project?.panelInstanceId ?? "");
    });
    // Hide the bottom strip after FlexLayout settles panel activation during reset.
    await workbenchLayoutControl.setEdgeCollapsed("bottom", true);
    logsLayoutControl.resetToDefault();
  } catch (error) {
    showWorkbenchLayoutError(error);
  } finally {
    workbenchLayoutController.completeLayoutReset(resetEpoch);
  }
}
