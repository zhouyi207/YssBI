import {
  Actions,
  GroupAction,
  BorderNode,
  DockLocation,
  Model,
  TabNode,
  TabSetNode,
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
  WORKBENCH_CONVERSATION_GROUP_ID,
  WORKBENCH_CONVERSATION_GROUP_ATTRIBUTES,
  WORKBENCH_BOTTOM_DEFAULT_ORDER,
  WORKBENCH_EDGE_GROUP_IDS,
  WORKBENCH_HOME_LOCATION,
  WORKBENCH_WORKSPACE_LAYOUT_ID,
  WORKBENCH_WORKSPACE_GROUP_ID,
} from "./workbenchLayoutDefaults";
import { isIntermediateLayoutAction } from "./layoutModelBinding";
export { WORKBENCH_ACTIVITY_GROUP_ID } from "./workbenchLayoutDefaults";
export function canFloatWorkbenchPanel(metadata: WorkbenchPanelMetadata): boolean {
  return (
    metadata.role !== "conversation" &&
    !isWorkbenchActivityMetadata(metadata) &&
    !isWorkbenchPersistentViewMetadata(metadata)
  );
}
export function canMoveWorkbenchPanel(
  metadata: WorkbenchPanelMetadata,
  targetGroupId: string,
  targetPosition?: "grid" | "conversation" | "top" | "bottom" | "left" | "right" | "float",
): boolean {
  if (targetGroupId === WORKBENCH_WORKSPACE_GROUP_ID) return false;
  if (targetPosition === "float") return canFloatWorkbenchPanel(metadata);
  if (metadata.role === "conversation") return targetGroupId === WORKBENCH_CONVERSATION_GROUP_ID;
  if (targetGroupId === WORKBENCH_CONVERSATION_GROUP_ID) return false;
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
    metadata.role !== "conversation" &&
    referenceGroupId !== WORKBENCH_WORKSPACE_GROUP_ID &&
    referenceGroupId !== WORKBENCH_CONVERSATION_GROUP_ID &&
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
    (metadata.role !== "view" || WORKBENCH_HOME_LOCATION[metadata.viewId] !== "bottom")
  );
}
/** One business tabset is active across conversations, the workspace and floats. */
export function getActiveWorkbenchTabset(model: Model): TabSetNode | undefined {
  let active: TabSetNode | undefined;
  model.visitNodes((node) => {
    if (
      node instanceof TabSetNode &&
      node.getId() !== WORKBENCH_WORKSPACE_GROUP_ID &&
      node.getSelectedNode() &&
      node.isActive()
    )
      active = node;
  });
  return active;
}

export function configureWorkbenchModel(model: Model): void {
  model.setOnCreateTabSet((tab) =>
    tab?.getConfig()?.metadata?.role === "conversation"
      ? WORKBENCH_CONVERSATION_GROUP_ATTRIBUTES
      : {},
  );
  let reconciling = false;
  let before: TabSetNode | undefined;
  const actionTarget = (action: Action): TabSetNode | undefined => {
    if (action instanceof GroupAction) {
      for (const child of [...action.actions].reverse()) {
        const target = actionTarget(child);
        if (target) return target;
      }
      return;
    }
    let node: Node | undefined;
    if (action.type === Actions.SELECT_TAB) node = model.getNodeById(action.data.tabNode);
    else if (action.type === Actions.SET_ACTIVE_TABSET)
      node = model.getNodeById(action.data.tabsetNode);
    else if ([Actions.POPOUT_TAB, Actions.POPOUT_TABSET].includes(action.type))
      node = model.getNodeById(action.data.node);
    else if (action.type === Actions.MOVE_NODE && action.data.select !== false)
      node = model.getNodeById(action.data.fromNode);
    else if (action.type === Actions.MOVE_FLOAT_TO_FRONT)
      node =
        model.getActiveTabset(action.data.layoutId) ??
        model.getFirstTabSet(model.getRootRow(action.data.layoutId));
    else if (action.type === Actions.DOCK_FLOAT_TO_LAYOUT)
      node = model.getNodeById(action.data.toNode);
    if (node instanceof TabNode) node = node.getParent();
    return node instanceof TabSetNode && node.getId() !== WORKBENCH_WORKSPACE_GROUP_ID
      ? node
      : undefined;
  };
  const reconcile = (action?: Action) => {
    if (reconciling || (action && isIntermediateLayoutAction(action))) return;
    const groups: TabSetNode[] = [];
    model.visitNodes((node) => {
      if (
        node instanceof TabSetNode &&
        node.getId() !== WORKBENCH_WORKSPACE_GROUP_ID &&
        node.getTabNodes().length
      )
        groups.push(node);
    });
    const requested = action && actionTarget(action);
    const active =
      (requested && groups.includes(requested) ? requested : undefined) ??
      (before && groups.includes(before) ? before : undefined) ??
      getActiveWorkbenchTabset(model) ??
      groups[0];
    const actions: Action[] = groups.flatMap((group) =>
      group.getSelectedNode() ? [] : [Actions.selectTab(group.getTabNodes()[0].getId())],
    );
    const layouts = new Set([
      Model.MAIN_LAYOUT_ID,
      WORKBENCH_WORKSPACE_LAYOUT_ID,
      ...groups.map((group) => group.getLayoutId()),
    ]);
    for (const layoutId of layouts) {
      const desired = active?.getLayoutId() === layoutId ? active : undefined;
      if (model.getActiveTabset(layoutId) !== desired || actions.length)
        actions.push(Actions.setActiveTabset(desired?.getId(), layoutId));
    }
    if (actions.length) {
      reconciling = true;
      try {
        // Complete the triggering action before its binding publishes the final projection.
        model.doAction(Actions.group(actions).setAdjusting(action !== undefined));
      } finally {
        reconciling = false;
      }
    }
  };
  reconcile();
  model.addChangeListener({
    onBeforeAction: (action) => {
      if (!reconciling && !isIntermediateLayoutAction(action))
        before = getActiveWorkbenchTabset(model);
    },
    onAfterAction: reconcile,
  });
  model.setOnAllowDrop((source, drop) => {
    const tabs: TabNode[] = [];
    const visit = (node: Node): void => {
      if (node instanceof TabNode) tabs.push(node);
      else node.getChildren().forEach(visit);
    };
    visit(source);
    const target = drop.node instanceof TabNode ? drop.node.getParent() : drop.node;
    if (!target || tabs.length === 0) return false;
    if (target === model.getRootRow() || target.getId() === WORKBENCH_WORKSPACE_GROUP_ID)
      return false;
    return tabs.every((tab) => {
      const metadata: unknown = tab.getConfig()?.metadata;
      if (!isWorkbenchPanelMetadata(metadata)) return false;
      const floating =
        target.getLayoutId() !== Model.MAIN_LAYOUT_ID &&
        target.getLayoutId() !== WORKBENCH_WORKSPACE_LAYOUT_ID;
      if (floating && !canFloatWorkbenchPanel(metadata)) return false;
      if (drop.location !== DockLocation.CENTER)
        return canSplitWorkbenchPanel(metadata, target.getId()) && !(target instanceof BorderNode);
      return canMoveWorkbenchPanel(metadata, target.getId(), floating ? "float" : undefined);
    });
  });
}
