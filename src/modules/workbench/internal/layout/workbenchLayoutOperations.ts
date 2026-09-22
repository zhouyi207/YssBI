import {
  Actions,
  BorderNode,
  DockLocation,
  Model,
  TabNode,
  TabSetNode,
  type Action,
  type IJsonTabNode,
} from "flexlayout-react";
import { resultReferenceKey } from "@/shared/types/domain/result";
import {
  canMoveWorkbenchPanel,
  canFloatWorkbenchPanel,
  getActiveWorkbenchTabset,
  canRemoveWorkbenchPanel,
  canSplitWorkbenchPanel,
  hasWorkbenchPanelCloseButton,
} from "./workbenchActivityGroup";
import { WORKBENCH_HOME_LOCATION, WORKBENCH_EDGE_SIZES } from "./workbenchLayoutDefaults";
import {
  componentForWorkbenchMetadata,
  isWorkbenchPanelMetadata,
  isWorkbenchPersistentViewMetadata,
  type WorkbenchPanelMetadata,
} from "./workbenchPanelModel";
import {
  WorkbenchLayoutError,
  type WorkbenchEdgePosition,
  type WorkbenchPanelInfo,
  type WorkbenchGroupInfo,
  type ConfigureWorkbenchEdgeRequest,
  type ConfiguredWorkbenchEdgeState,
  type EnsureViewRequest,
  type EnsurePluginViewRequest,
  type OpenEditorRequest,
  type UpsertResultRequest,
  type MoveWorkbenchPanelRequest,
  type SplitWorkbenchPanelRequest,
} from "./workbenchTypes";

export function readMetadata(node: TabNode | undefined): WorkbenchPanelMetadata | undefined {
  const metadata: unknown = node?.getConfig()?.metadata;
  return isWorkbenchPanelMetadata(metadata) ? metadata : undefined;
}
export function metadataEqual(a: WorkbenchPanelMetadata, b: WorkbenchPanelMetadata): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}
export function modelTabs(model: Model): TabNode[] {
  const tabs: TabNode[] = [];
  model.visitNodes((node) => {
    if (node instanceof TabNode) tabs.push(node);
  });
  return tabs;
}
export function modelGroups(model: Model): (TabSetNode | BorderNode)[] {
  const groups: (TabSetNode | BorderNode)[] = [];
  model.visitNodes((node) => {
    // The native model keeps an empty central drop target; it is not an open group.
    if ((node instanceof TabSetNode || node instanceof BorderNode) && node.getTabNodes().length)
      groups.push(node);
  });
  return groups;
}
export function panelIsVisible(tab: TabNode): boolean {
  const parent = tab.getParent();
  return (
    (parent instanceof TabSetNode || parent instanceof BorderNode) &&
    parent.getSelectedNode() === tab &&
    (!(parent instanceof BorderNode) || parent.isShowing()) &&
    (!tab.getModel().getMaximizedTabset(tab.getLayoutId()) ||
      parent instanceof BorderNode ||
      tab.getModel().getMaximizedTabset(tab.getLayoutId()) === parent)
  );
}
function nodeLocation(group: TabSetNode | BorderNode): WorkbenchGroupInfo["location"] {
  if (group.getLayoutId() !== Model.MAIN_LAYOUT_ID)
    return { type: "float", layoutId: group.getLayoutId() };
  return group instanceof BorderNode
    ? { type: "edge", position: group.getLocation().getName() as WorkbenchEdgePosition }
    : { type: "grid" };
}
export class WorkbenchModelOperations {
  constructor(readonly model: Model) {}
  serialize = () => structuredClone(this.model.toJson());
  private tab = (id: string): TabNode | undefined => {
    const node = this.model.getNodeById(id);
    return node instanceof TabNode ? node : undefined;
  };
  private group = (id: string): TabSetNode | BorderNode => {
    const node = this.model.getNodeById(id);
    if (!(node instanceof TabSetNode || node instanceof BorderNode))
      throw new WorkbenchLayoutError("group_not_found", { groupId: id });
    return node;
  };
  private activeTab = (): TabNode | undefined =>
    getActiveWorkbenchTabset(this.model)?.getSelectedNode();
  private panelInfo = (
    tab: TabNode | undefined,
    active: TabNode | undefined,
  ): WorkbenchPanelInfo | undefined => {
    const metadata = readMetadata(tab);
    const parent = tab?.getParent();
    if (!tab || !metadata || !(parent instanceof TabSetNode || parent instanceof BorderNode))
      return undefined;
    return {
      panelInstanceId: tab.getId(),
      groupId: parent.getId(),
      component: componentForWorkbenchMetadata(metadata),
      title: tab.getName(),
      metadata,
      active: active === tab,
      visible: panelIsVisible(tab),
      location: nodeLocation(parent),
    };
  };
  getPanel = (id: string): WorkbenchPanelInfo | undefined =>
    this.panelInfo(this.tab(id), this.activeTab());
  getActivePanel = () => {
    const tab = this.activeTab();
    return this.panelInfo(tab, tab);
  };
  listPanels = (): WorkbenchPanelInfo[] => {
    const active = this.activeTab();
    return modelTabs(this.model).flatMap((tab) => {
      const panel = this.panelInfo(tab, active);
      return panel ? [panel] : [];
    });
  };
  listGroups = (): WorkbenchGroupInfo[] => {
    const activeGroup = this.activeTab()?.getParent();
    return modelGroups(this.model).map((group) => ({
      groupId: group.getId(),
      panelInstanceIds: group.getTabNodes().map((tab) => tab.getId()),
      activePanelInstanceId: group.getSelectedNode()?.getId(),
      active: activeGroup === group,
      location: nodeLocation(group),
    }));
  };
  listGroupPanels = (id: string): WorkbenchPanelInfo[] => {
    const group = this.model.getNodeById(id);
    if (!(group instanceof BorderNode || group instanceof TabSetNode)) return [];
    const active = this.activeTab();
    return group.getTabNodes().flatMap((tab) => {
      const panel = this.panelInfo(tab, active);
      return panel ? [panel] : [];
    });
  };
  getEdgeState = (position: WorkbenchEdgePosition) => {
    const node = this.model.getNodeById("border_" + position);
    const exists = node instanceof BorderNode;
    return {
      position,
      exists,
      groupId: exists ? node.getId() : undefined,
      visible: exists && node.isShowing() && (!node.isAutoHide() || node.getTabNodes().length > 0),
      collapsed: !exists || node.getSelected() < 0,
    };
  };
  ensureCentralGroup = (): string => {
    const group = this.model.getActiveTabset() ?? this.model.getFirstTabSet();
    if (!group) throw new WorkbenchLayoutError("group_not_found", { reason: "missing_center" });
    return group.getId();
  };
  private add = (
    metadata: WorkbenchPanelMetadata,
    title: string,
    groupId: string,
    index = -1,
  ): WorkbenchPanelInfo => {
    const id = crypto.randomUUID();
    const json: IJsonTabNode = {
      type: "tab",
      id,
      name: title,
      component: componentForWorkbenchMetadata(metadata),
      config: { metadata },
      enableClose: hasWorkbenchPanelCloseButton(metadata),
      enableFloat: canFloatWorkbenchPanel(metadata),
      enableDrag: !isWorkbenchPersistentViewMetadata(metadata),
    };
    this.model.doAction(
      Actions.addTab(json, this.group(groupId).getId(), DockLocation.CENTER, index, false),
    );
    this.activate(id);
    const panel = this.getPanel(id);
    if (!panel) throw new WorkbenchLayoutError("panel_open_failed");
    return panel;
  };
  ensureView = (request: EnsureViewRequest): WorkbenchPanelInfo => {
    const existing = this.listPanels().find(
      (panel) => panel.metadata.role === "view" && panel.metadata.viewId === request.viewId,
    );
    if (existing) {
      this.reveal(existing.panelInstanceId);
      return this.getPanel(existing.panelInstanceId)!;
    }
    return this.add(
      { role: "view", viewId: request.viewId },
      request.title,
      "border_" + WORKBENCH_HOME_LOCATION[request.viewId],
    );
  };
  ensurePluginView = (request: EnsurePluginViewRequest): WorkbenchPanelInfo => {
    const existing = this.listPanels().find(
      (panel) =>
        panel.metadata.role === "plugin" &&
        panel.metadata.pluginId === request.pluginId &&
        panel.metadata.viewId === request.viewId,
    );
    if (existing) {
      this.reveal(existing.panelInstanceId);
      return this.getPanel(existing.panelInstanceId)!;
    }
    return this.add(
      { role: "plugin", ...request },
      request.title,
      request.location === "sidebar" ? "border_left" : this.ensureCentralGroup(),
    );
  };
  openEditor = (request: OpenEditorRequest): WorkbenchPanelInfo => {
    const existing =
      request.mode === "reuse-resource"
        ? this.listPanels().find(
            (panel) =>
              panel.metadata.role === "editor" &&
              panel.metadata.resourceRef === request.resourceRef &&
              panel.metadata.resourceKind === request.resourceKind,
          )
        : undefined;
    if (existing) {
      const tab = this.tab(existing.panelInstanceId)!;
      this.model.doAction(
        Actions.updateNodeAttributes(tab.getId(), {
          name: request.title,
        }),
      );
      this.reveal(existing.panelInstanceId);
      return this.getPanel(existing.panelInstanceId)!;
    }
    const group = request.targetGroupId
      ? this.group(request.targetGroupId)
      : (getActiveWorkbenchTabset(this.model) ?? this.group(this.ensureCentralGroup()));
    if (!(group instanceof TabSetNode)) throw new WorkbenchLayoutError("group_not_found");
    return this.add(
      {
        role: "editor",
        resourceRef: request.resourceRef,
        resourceKind: request.resourceKind,
      },
      request.title,
      group.getId(),
      request.index,
    );
  };
  upsertResult = (request: UpsertResultRequest): WorkbenchPanelInfo => {
    const key = resultReferenceKey(request.reference);
    const existing = this.listPanels().find(
      (panel) =>
        panel.metadata.role === "result" && resultReferenceKey(panel.metadata.reference) === key,
    );
    if (existing) {
      this.reveal(existing.panelInstanceId);
      return this.getPanel(existing.panelInstanceId)!;
    }
    return this.add(
      { role: "result", ...request },
      request.title,
      "border_" + WORKBENCH_HOME_LOCATION.result,
    );
  };
  activate = (id: string): boolean => {
    const tab = this.tab(id);
    if (!tab) return false;
    const parent = tab.getParent();
    const actions: Action[] = [];
    const maximized = this.model.getMaximizedTabset(tab.getLayoutId());
    if (parent instanceof TabSetNode && maximized && maximized !== parent)
      actions.push(Actions.maximizeToggle(maximized.getId()));
    if (
      (parent instanceof TabSetNode || parent instanceof BorderNode) &&
      parent.getSelectedNode() !== tab
    )
      actions.push(Actions.selectTab(id));
    if (parent instanceof BorderNode && !parent.isShowing())
      actions.push(Actions.updateNodeAttributes(parent.getId(), { show: true }));
    if (parent instanceof TabSetNode && this.model.getActiveTabset(parent.getLayoutId()) !== parent)
      actions.push(Actions.setActiveTabset(parent.getId(), parent.getLayoutId()));
    if (tab.getLayoutId() !== Model.MAIN_LAYOUT_ID)
      actions.push(Actions.movePopoutToFront(tab.getLayoutId()));
    if (actions.length) this.model.doAction(Actions.group(actions));
    return true;
  };
  reveal = this.activate;
  move = (request: MoveWorkbenchPanelRequest): boolean => {
    const tab = this.tab(request.panelInstanceId);
    const metadata = readMetadata(tab);
    const target = this.group(request.groupId);
    const location = nodeLocation(target);
    if (
      !tab ||
      !metadata ||
      !canMoveWorkbenchPanel(
        metadata,
        target.getId(),
        location.type === "edge" ? location.position : location.type,
      )
    )
      return false;
    const index =
      request.index ?? (tab.getParent() === target ? target.getTabNodes().indexOf(tab) : -1);
    if (tab.getParent() !== target || index !== target.getTabNodes().indexOf(tab))
      this.model.doAction(
        Actions.moveNode(tab.getId(), target.getId(), DockLocation.CENTER, index, false),
      );
    if (request.activate !== false) this.activate(tab.getId());
    return true;
  };
  split = (request: SplitWorkbenchPanelRequest): boolean => {
    const tab = this.tab(request.panelInstanceId);
    const metadata = readMetadata(tab);
    const target = this.group(request.referenceGroupId);
    if (
      !tab ||
      !metadata ||
      !(target instanceof TabSetNode) ||
      !canSplitWorkbenchPanel(metadata, target.getId())
    )
      return false;
    this.model.doAction(
      Actions.moveNode(
        tab.getId(),
        target.getId(),
        {
          top: DockLocation.TOP,
          bottom: DockLocation.BOTTOM,
          left: DockLocation.LEFT,
          right: DockLocation.RIGHT,
        }[request.direction],
        -1,
        request.activate !== false,
      ),
    );
    if (request.activate !== false) this.activate(tab.getId());
    return true;
  };
  floatPanel = (id: string): boolean => {
    const tab = this.tab(id);
    const metadata = readMetadata(tab);
    if (
      !tab ||
      !metadata ||
      !canFloatWorkbenchPanel(metadata) ||
      !(tab.getParent() instanceof TabSetNode)
    )
      return false;
    this.model.doAction(Actions.popoutTab(id, "float"));
    this.activate(id);
    return true;
  };
  floatGroup = (id: string): boolean => {
    const group = this.group(id);
    if (
      !(group instanceof TabSetNode) ||
      !group.getTabNodes().length ||
      !group.getTabNodes().every((tab) => {
        const metadata = readMetadata(tab);
        return metadata && canFloatWorkbenchPanel(metadata);
      })
    )
      return false;
    this.model.doAction(Actions.popoutTabset(id, "float"));
    const selected = group.getSelectedNode();
    if (selected) this.activate(selected.getId());
    return true;
  };
  dockFloat = (layoutId: string): boolean => {
    if (layoutId === Model.MAIN_LAYOUT_ID) return false;
    const row = this.model.getRootRow(layoutId);
    if (!row) return false;
    const selected =
      this.model.getActiveTabset(layoutId)?.getSelectedNode() ??
      this.model.getFirstTabSet(row)?.getSelectedNode();
    const maximized = this.model.getMaximizedTabset();
    this.model.doAction(
      Actions.group([
        ...(maximized ? [Actions.maximizeToggle(maximized.getId())] : []),
        Actions.dockFloatToLayout(
          layoutId,
          this.model.getRootRow()!.getId(),
          DockLocation.RIGHT,
          -1,
        ),
      ]),
    );
    if (selected) this.activate(selected.getId());
    return true;
  };
  configureEdge = (request: ConfigureWorkbenchEdgeRequest): ConfiguredWorkbenchEdgeState => {
    const border = this.group("border_" + request.position);
    if (!(border instanceof BorderNode) || !Number.isFinite(request.size) || request.size <= 0)
      throw new WorkbenchLayoutError("layout_restore_failed", { reason: "invalid_edge" });
    this.model.doAction(
      Actions.updateNodeAttributes(border.getId(), {
        size: request.size,
        show: true,
        selected: request.collapsed ? -1 : Math.max(0, border.getSelected()),
      }),
    );
    return {
      ...this.getEdgeState(request.position),
      exists: true,
      groupId: border.getId(),
      size: request.size,
    };
  };
  setEdgeCollapsed = (position: WorkbenchEdgePosition, collapsed: boolean): boolean => {
    const border = this.model.getNodeById("border_" + position);
    if (!(border instanceof BorderNode)) return false;
    const current = border.getSelected();
    this.model.doAction(
      Actions.updateNodeAttributes(border.getId(), {
        selected: collapsed ? -1 : Math.min(border.getTabNodes().length - 1, Math.max(0, current)),
        show: true,
      }),
    );
    return true;
  };
  setEdgeSize = (position: WorkbenchEdgePosition, size: number): boolean => {
    const state = this.getEdgeState(position);
    if (!state.exists) return false;
    this.configureEdge({ position, size, collapsed: state.collapsed });
    return true;
  };
  remapResource = (from: string, to: string): number => {
    let count = 0;
    for (const tab of modelTabs(this.model)) {
      const metadata = readMetadata(tab);
      if (metadata?.role !== "editor" || metadata.resourceRef !== from) continue;
      this.model.doAction(
        Actions.updateNodeAttributes(tab.getId(), {
          config: { ...tab.getConfig(), metadata: { ...metadata, resourceRef: to } },
        }),
      );
      count++;
    }
    return count;
  };
  removePanels = (ids: readonly string[]): void => {
    const tabs = [...new Set(ids)].flatMap((id) => {
      const tab = this.tab(id);
      return tab ? [tab] : [];
    });
    for (const tab of tabs) {
      const metadata = readMetadata(tab);
      if (!metadata || !canRemoveWorkbenchPanel(metadata))
        throw new WorkbenchLayoutError("layout_restore_failed", { reason: "fixed_panel" });
    }
    if (tabs.length)
      this.model.doAction(Actions.group(tabs.map((tab) => Actions.deleteTab(tab.getId()))));
  };
}

export function readSerializedEdge(
  layout: ReturnType<Model["toJson"]>,
  position: WorkbenchEdgePosition,
) {
  const border = layout.borders?.find((node) => node.location === position);
  if (!border) return undefined;
  return {
    size: border.size ?? WORKBENCH_EDGE_SIZES[position as keyof typeof WORKBENCH_EDGE_SIZES] ?? 200,
    collapsed: (border.selected ?? -1) < 0,
    activePanelId: border.children?.[border.selected ?? -1]?.id,
  };
}
