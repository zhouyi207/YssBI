import {
  Actions,
  BorderNode,
  DockLocation,
  TabNode,
  TabSetNode,
  type Model,
  type Node,
  type Action,
} from "flexlayout-react";
import {
  isWorkbenchActivityMetadata,
  isWorkbenchPanelMetadata,
  isWorkbenchPersistentViewMetadata,
  type WorkbenchPanelMetadata,
} from "./workbenchPanelModel";
import {
  WORKBENCH_ACTIVITY_GROUP_ID,
  WORKBENCH_BOTTOM_DEFAULT_ORDER,
  WORKBENCH_EDGE_GROUP_IDS,
  WORKBENCH_HOME_EDGE,
} from "./workbenchLayoutDefaults";
export { WORKBENCH_ACTIVITY_GROUP_ID } from "./workbenchLayoutDefaults";
export function canMoveWorkbenchPanel(
  metadata: WorkbenchPanelMetadata,
  targetGroupId: string,
  targetPosition?: "grid" | "top" | "bottom" | "left" | "right",
): boolean {
  if (targetGroupId === WORKBENCH_EDGE_GROUP_IDS.bottom || targetPosition === "bottom")
    return (
      metadata.role === "view" &&
      WORKBENCH_BOTTOM_DEFAULT_ORDER.some((viewId) => viewId === metadata.viewId)
    );
  if (isWorkbenchActivityMetadata(metadata)) return targetGroupId === WORKBENCH_ACTIVITY_GROUP_ID;
  if (isWorkbenchPersistentViewMetadata(metadata))
    return targetPosition === "right" || targetGroupId === WORKBENCH_EDGE_GROUP_IDS.right;
  return targetGroupId !== WORKBENCH_ACTIVITY_GROUP_ID;
}
export function canSplitWorkbenchPanel(
  metadata: WorkbenchPanelMetadata,
  referenceGroupId: string,
): boolean {
  return (
    !isWorkbenchActivityMetadata(metadata) &&
    !isWorkbenchPersistentViewMetadata(metadata) &&
    referenceGroupId !== WORKBENCH_ACTIVITY_GROUP_ID
  );
}
export function canRemoveWorkbenchPanel(metadata: WorkbenchPanelMetadata): boolean {
  return (
    metadata.role === "plugin" ||
    (!isWorkbenchActivityMetadata(metadata) && !isWorkbenchPersistentViewMetadata(metadata))
  );
}
export function hasWorkbenchPanelCloseButton(metadata: WorkbenchPanelMetadata): boolean {
  return (
    canRemoveWorkbenchPanel(metadata) &&
    (metadata.role !== "view" || WORKBENCH_HOME_EDGE[metadata.viewId] !== "bottom")
  );
}
export function configureWorkbenchModel(model: Model): void {
  const ensureActiveTabset = (initial = false) => {
    if (!initial && model.getActiveTabset()?.getSelectedNode()) return;
    const groups: TabSetNode[] = [];
    model.visitNodes((node) => {
      if (node instanceof TabSetNode && node.getTabNodes().length) groups.push(node);
    });
    const actions: Action[] = groups.flatMap((group) =>
      group.getSelectedNode() ? [] : [Actions.selectTab(group.getTabNodes()[0].getId())],
    );
    const active = model.getActiveTabset();
    const next = active && groups.includes(active) ? active : groups[0];
    if (next && (next !== active || actions.length))
      actions.push(Actions.setActiveTabset(next.getId()));
    if (actions.length) model.doAction(Actions.group(actions));
  };
  ensureActiveTabset(true);
  model.addChangeListener({ onAfterAction: () => ensureActiveTabset() });
  model.setOnAllowDrop((source, drop) => {
    const tabs: TabNode[] = [];
    const visit = (node: Node): void => {
      if (node instanceof TabNode) tabs.push(node);
      else node.getChildren().forEach(visit);
    };
    visit(source);
    const target = drop.node instanceof TabNode ? drop.node.getParent() : drop.node;
    if (!target || tabs.length === 0) return false;
    return tabs.every((tab) => {
      const metadata: unknown = tab.getConfig()?.metadata;
      if (!isWorkbenchPanelMetadata(metadata)) return false;
      if (drop.location !== DockLocation.CENTER)
        return canSplitWorkbenchPanel(metadata, target.getId()) && !(target instanceof BorderNode);
      return canMoveWorkbenchPanel(metadata, target.getId());
    });
  });
}
