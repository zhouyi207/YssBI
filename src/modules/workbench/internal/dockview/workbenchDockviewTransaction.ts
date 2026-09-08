import type {
  AddGroupOptions,
  AddPanelOptions,
  DockviewApi,
  DockviewGroupPanel,
  IDockviewGroupPanel,
  SerializedDockview,
} from "dockview-react";

import { canMoveWorkbenchPanel, canRemoveWorkbenchPanel } from "./workbenchActivityGroup";
import { WORKBENCH_HOME_EDGE } from "./workbenchDockviewDefaults";
import {
  componentForWorkbenchMetadata,
  isWorkbenchPanelMetadata,
  type WorkbenchComponentId,
  type WorkbenchPanelMetadata,
  type WorkbenchPanelParams,
} from "./workbenchPanelModel";
import type {
  ConfiguredWorkbenchEdgeState,
  ConfigureWorkbenchEdgeRequest,
  EnsureViewRequest,
  EnsurePluginViewRequest,
  MoveWorkbenchPanelRequest,
  WorkbenchEdgePosition,
  WorkbenchGroupInfo,
  WorkbenchPanelInfo,
} from "./workbenchTypes";
import { WorkbenchLayoutError } from "./workbenchTypes";

import {
  type WorkbenchLocation,
  type UnknownRecord,
  EDGE_POSITIONS,
  DEFAULT_EDGE_SIZES,
  isRecord,
  cloneMetadata,
  readMetadata,
  panelParams,
  readLocation,
  edgeSize,
  throwAsLayoutError,
  requireValidMetadata,
  updatePanelMetadata,
  defaultHeaderPositionForEdge,
  generatedId,
  configuredEdgeId,
  revealPanel,
  configureEdgeLive,
  remappedMetadata,
} from "./workbenchDockviewOperations";
import type { WorkbenchDockviewTransaction, WorkbenchLayoutTransaction } from "./workbenchTypes";

interface ShadowPanel {
  readonly id: string;
  component: string;
  title: string | undefined;
  params: UnknownRecord;
  metadata: WorkbenchPanelMetadata | undefined;
  groupId: string;
  active: boolean;
}

interface ShadowGroup {
  readonly id: string;
  readonly location: WorkbenchLocation;
  readonly panelIds: string[];
  activePanelId: string | undefined;
  active: boolean;
  headerPosition?: "top" | "bottom" | "left" | "right";
}

interface ShadowEdge {
  readonly position: WorkbenchEdgePosition;
  readonly groupId: string;
  visible: boolean;
  collapsed: boolean;
  size: number;
  headerPosition?: "top" | "bottom" | "left" | "right";
}

type BufferedCommand =
  | { readonly kind: "add-grid"; readonly groupId: string }
  | {
      readonly kind: "add-edge";
      readonly position: WorkbenchEdgePosition;
      readonly groupId: string;
      readonly size: number;
      readonly collapsed: boolean;
    }
  | {
      readonly kind: "add-panel";
      readonly panelId: string;
      readonly groupId: string;
      readonly component: WorkbenchComponentId;
      readonly title: string;
      readonly metadata: WorkbenchPanelMetadata;
      readonly index?: number;
    }
  | {
      readonly kind: "update-panel";
      readonly panelId: string;
      readonly metadata: WorkbenchPanelMetadata;
      readonly title?: string;
      readonly updateTitle: boolean;
      readonly pinned?: boolean;
    }
  | {
      readonly kind: "move";
      readonly panelId: string;
      readonly groupId: string;
      readonly index?: number;
      readonly activate: boolean;
    }
  | {
      readonly kind: "configure-edge";
      readonly request: ConfigureWorkbenchEdgeRequest;
    }
  | { readonly kind: "reveal"; readonly panelId: string }
  | { readonly kind: "activate"; readonly panelId: string }
  | {
      readonly kind: "remove";
      readonly panelId: string;
      readonly deferUntilFinal: boolean;
    };

interface MutableSerializedLeaf {
  readonly type: "leaf";
  readonly size?: number;
  readonly data: UnknownRecord;
}

interface MutableSerializedBranch {
  readonly type: "branch";
  readonly size?: number;
  readonly data: MutableSerializedNode[];
}

type MutableSerializedNode = MutableSerializedLeaf | MutableSerializedBranch;

interface MutableSerializedLayout {
  grid: {
    root: MutableSerializedNode;
    height: number;
    width: number;
    orientation: unknown;
  };
  panels: Record<string, UnknownRecord>;
  activeGroup?: string;
  edgeGroups?: Partial<Record<WorkbenchEdgePosition, UnknownRecord>>;
  [key: string]: unknown;
}

export class PendingWorkbenchTransaction {
  private readonly panels = new Map<string, ShadowPanel>();
  private readonly groups = new Map<string, ShadowGroup>();
  private readonly groupOrder: string[] = [];
  private readonly edges = new Map<WorkbenchEdgePosition, ShadowEdge>();
  private readonly commands: BufferedCommand[] = [];
  private readonly violations: string[] = [];
  private readonly baseLayout: SerializedDockview;
  private readonly baseFingerprint: string;

  constructor(api: DockviewApi) {
    this.baseLayout = structuredClone(api.toJSON());
    this.baseFingerprint = JSON.stringify(this.baseLayout);

    for (const group of api.groups) {
      const location = readLocation(group);
      if (!location) continue;
      const shadow: ShadowGroup = {
        id: group.id,
        location,
        panelIds: group.panels.map((panel) => panel.id),
        activePanelId: group.activePanel?.id,
        active: api.activeGroup?.id === group.id,
        headerPosition: group.api.getHeaderPosition(),
      };
      this.groups.set(group.id, shadow);
      this.groupOrder.push(group.id);
    }

    for (const panel of api.panels) {
      const group = this.groups.get(panel.group.id);
      if (!group) continue;
      const metadata = readMetadata(panel);
      this.panels.set(panel.id, {
        id: panel.id,
        component: panel.api.component,
        title: panel.title,
        params: { ...panelParams(panel) },
        metadata,
        groupId: group.id,
        active: panel.api.isActive,
      });
    }

    for (const position of EDGE_POSITIONS) {
      const group = api.getEdgeGroup(position);
      if (!group) continue;
      this.edges.set(position, {
        position,
        groupId: group.id,
        visible: api.isEdgeGroupVisible(position),
        collapsed: group.isCollapsed(),
        size: edgeSize(api, position, group, this.baseLayout),
        headerPosition: group.getHeaderPosition(),
      });
    }
  }

  readonly publication: WorkbenchDockviewTransaction = {
    listPanels: () => this.listPanels(),
    remapResource: (from, to) => this.remapResource(from, to),
    removePanels: (panelInstanceIds) => this.removePanels(panelInstanceIds),
  };

  readonly layout: WorkbenchLayoutTransaction = {
    serialize: () => this.serialize(),
    getPanel: (panelInstanceId) => this.getPanel(panelInstanceId),
    getActivePanel: () => this.getActivePanel(),
    listPanels: () => this.listPanels(),
    listGroups: () => this.listGroups(),
    listGroupPanels: (groupId) => this.listGroupPanels(groupId),
    ensureCentralGroup: () => this.ensureCentralGroup(),
    ensureView: (request) => this.ensureView(request),
    ensurePluginView: (request) => this.ensurePluginView(request),
    move: (request) => this.move(request),
    configureEdge: (request) => this.configureEdge(request),
    activate: (panelInstanceId) => this.activate(panelInstanceId),
    removePanels: (panelInstanceIds) => this.removePanels(panelInstanceIds),
  };

  hasBufferedCommands(): boolean {
    return this.commands.length > 0;
  }

  validate(api: DockviewApi): void {
    if (JSON.stringify(api.toJSON()) !== this.baseFingerprint) {
      this.fail("stale_transaction");
    }
    if (this.violations.length > 0) this.fail(this.violations[0] ?? "invalid_shadow");

    const seenPanelIds = new Set<string>();
    for (const groupId of this.groupOrder) {
      const group = this.groups.get(groupId);
      if (!group) this.fail("missing_group");
      if (
        (group.panelIds.length === 0) !== (group.activePanelId === undefined) ||
        (group.activePanelId !== undefined && !group.panelIds.includes(group.activePanelId))
      ) {
        this.fail("invalid_active_panel");
      }
      for (const panelId of group.panelIds) {
        if (seenPanelIds.has(panelId)) this.fail("duplicate_panel");
        seenPanelIds.add(panelId);
        if (this.panels.get(panelId)?.groupId !== groupId) this.fail("invalid_group_membership");
      }
    }
    if (seenPanelIds.size !== this.panels.size) this.fail("orphan_panel");

    const activeGroups = [...this.groups.values()].filter((group) => group.active);
    const shouldHaveActiveGroup = [...this.groups.values()].some(
      (group) => group.location.type === "grid" || group.activePanelId !== undefined,
    );
    if (activeGroups.length > 1 || (shouldHaveActiveGroup && activeGroups.length !== 1)) {
      this.fail("invalid_active_state");
    }
    const activeGroup = activeGroups[0];
    const expectedActivePanelId = activeGroup?.activePanelId;
    for (const panel of this.panels.values()) {
      if (panel.active !== (panel.id === expectedActivePanelId)) {
        this.fail("invalid_active_state");
      }
    }

    const viewIds = new Set<string>();
    const resultKeys = new Set<string>();
    for (const panel of this.panels.values()) {
      if (!panel.metadata) continue;
      if (!isWorkbenchPanelMetadata(panel.metadata)) this.fail("invalid_panel_metadata");
      if (panel.metadata.role === "view") {
        if (viewIds.has(panel.metadata.viewId)) this.fail("duplicate_view");
        viewIds.add(panel.metadata.viewId);
      }
      if (panel.metadata.role === "result") {
        if (resultKeys.has(panel.metadata.resultKey)) this.fail("duplicate_result");
        resultKeys.add(panel.metadata.resultKey);
      }
    }

    for (const [position, edge] of this.edges) {
      const group = this.groups.get(edge.groupId);
      if (!group || group.location.type !== "edge" || group.location.position !== position) {
        this.fail("invalid_edge_group");
      }
    }
  }

  apply(api: DockviewApi): void {
    if (this.commands.length === 0) return;
    for (const command of this.commands) {
      if (command.kind !== "remove" || !command.deferUntilFinal) {
        this.applyCommand(api, command);
      }
    }
    for (const command of this.commands) {
      if (command.kind === "remove" && command.deferUntilFinal) {
        this.applyCommand(api, command);
      }
    }
    this.ensureSelectionConsistency(api);
  }

  private getPanel(panelInstanceId: string): WorkbenchPanelInfo | undefined {
    const panel = this.panels.get(panelInstanceId);
    return panel ? this.toPanelInfo(panel) : undefined;
  }

  private getActivePanel(): WorkbenchPanelInfo | undefined {
    const panel = [...this.panels.values()].find((candidate) => candidate.active);
    return panel ? this.toPanelInfo(panel) : undefined;
  }

  private listPanels(): readonly WorkbenchPanelInfo[] {
    return [...this.panels.values()].flatMap((panel) => {
      const info = this.toPanelInfo(panel);
      return info ? [info] : [];
    });
  }

  private listGroups(): readonly WorkbenchGroupInfo[] {
    return this.groupOrder.flatMap((groupId) => {
      const group = this.groups.get(groupId);
      return group ? [this.toGroupInfo(group)] : [];
    });
  }

  private listGroupPanels(groupId: string): readonly WorkbenchPanelInfo[] {
    const group = this.groups.get(groupId);
    if (!group) return [];
    return group.panelIds.flatMap((panelId) => {
      const panel = this.panels.get(panelId);
      const info = panel ? this.toPanelInfo(panel) : undefined;
      return info ? [info] : [];
    });
  }

  private ensureCentralGroup(): string {
    const active = this.groupOrder
      .map((groupId) => this.groups.get(groupId))
      .find((group) => group?.active && group.location.type === "grid");
    if (active) return active.id;
    const existing = this.groupOrder
      .map((groupId) => this.groups.get(groupId))
      .find((group) => group?.location.type === "grid");
    if (existing) return existing.id;

    const groupId = this.uniqueId();
    this.groups.set(groupId, {
      id: groupId,
      location: { type: "grid" },
      panelIds: [],
      activePanelId: undefined,
      active: false,
    });
    this.groupOrder.push(groupId);
    this.setActiveGroupState(groupId);
    this.commands.push({ kind: "add-grid", groupId });
    return groupId;
  }

  private ensurePluginView(request: EnsurePluginViewRequest): WorkbenchPanelInfo {
    const existing = [...this.panels.values()].find(
      (panel) =>
        panel.metadata?.role === "plugin" &&
        panel.metadata.pluginId === request.pluginId &&
        panel.metadata.viewId === request.viewId,
    );
    if (existing) {
      this.reveal(existing.id);
      return this.toPanelInfo(existing)!;
    }
    const metadata = requireValidMetadata({ role: "plugin", ...request });
    const groupId =
      request.location === "sidebar" ? this.ensureEdge("left").groupId : this.ensureCentralGroup();
    const panelId = this.uniqueId();
    const panel: ShadowPanel = {
      id: panelId,
      component: "Plugin",
      title: request.title,
      params: { metadata: cloneMetadata(metadata) },
      metadata,
      groupId,
      active: false,
    };
    this.panels.set(panelId, panel);
    this.groups.get(groupId)!.panelIds.push(panelId);
    this.commands.push({
      kind: "add-panel",
      panelId,
      groupId,
      component: "Plugin",
      title: request.title,
      metadata,
    });
    this.reveal(panelId);
    return this.toPanelInfo(panel)!;
  }

  private ensureView(request: EnsureViewRequest): WorkbenchPanelInfo {
    const existing = [...this.panels.values()].find(
      (panel) => panel.metadata?.role === "view" && panel.metadata.viewId === request.viewId,
    );
    if (existing) {
      if (existing.title !== request.title) {
        existing.title = request.title;
        this.commands.push({
          kind: "update-panel",
          panelId: existing.id,
          metadata: cloneMetadata(existing.metadata as WorkbenchPanelMetadata),
          title: request.title,
          updateTitle: true,
        });
      }
      this.reveal(existing.id);
      const info = this.toPanelInfo(existing);
      if (info) return info;
      this.fail("invalid_panel_metadata");
    }

    const metadata = requireValidMetadata({ role: "view", viewId: request.viewId });
    const position = WORKBENCH_HOME_EDGE[request.viewId];
    const edge = this.ensureEdge(position);
    const panelId = this.uniqueId();
    const panel: ShadowPanel = {
      id: panelId,
      component: componentForWorkbenchMetadata(metadata),
      title: request.title,
      params: { metadata: cloneMetadata(metadata) },
      metadata,
      groupId: edge.groupId,
      active: false,
    };
    this.panels.set(panelId, panel);
    this.groups.get(edge.groupId)?.panelIds.push(panelId);
    this.commands.push({
      kind: "add-panel",
      panelId,
      groupId: edge.groupId,
      component: componentForWorkbenchMetadata(metadata),
      title: request.title,
      metadata,
    });
    this.reveal(panelId);
    const info = this.toPanelInfo(panel);
    if (info) return info;
    this.fail("invalid_panel_metadata");
  }

  private move(request: MoveWorkbenchPanelRequest): boolean {
    const panel = this.panels.get(request.panelInstanceId);
    const target = this.groups.get(request.groupId);
    if (!panel?.metadata || !target) return false;
    const source = this.groups.get(panel.groupId);
    if (!source) {
      this.violations.push("missing_source_group");
      return false;
    }
    const targetPosition =
      target.location.type === "edge" ? target.location.position : target.location.type;
    if (!canMoveWorkbenchPanel(panel.metadata, target.id, targetPosition)) {
      this.violations.push("activity_move_not_allowed");
      return false;
    }

    const currentIndex = source.panelIds.indexOf(panel.id);
    const maximumIndex =
      source === target ? Math.max(0, target.panelIds.length - 1) : target.panelIds.length;
    const requestedIndex = request.index ?? (source === target ? currentIndex : maximumIndex);
    if (!Number.isInteger(requestedIndex) || requestedIndex < 0 || requestedIndex > maximumIndex) {
      this.violations.push("invalid_index");
      return false;
    }
    if (source === target && requestedIndex === currentIndex) {
      if (request.activate !== false) this.activate(panel.id);
      return true;
    }

    const wasActive = panel.active;
    source.panelIds.splice(currentIndex, 1);
    if (source.activePanelId === panel.id) source.activePanelId = source.panelIds[0];
    if (source.panelIds.length === 0 && source.location.type === "grid") {
      this.deleteGroup(source.id);
    }
    panel.groupId = target.id;
    target.panelIds.splice(requestedIndex, 0, panel.id);
    if (!target.activePanelId) target.activePanelId = panel.id;
    this.commands.push({
      kind: "move",
      panelId: panel.id,
      groupId: target.id,
      index: requestedIndex,
      activate: request.activate !== false,
    });
    if (request.activate !== false) this.setActiveState(panel.id);
    else if (wasActive) this.setReplacementActiveState(source.id);
    return true;
  }

  private configureEdge(request: ConfigureWorkbenchEdgeRequest): ConfiguredWorkbenchEdgeState {
    if (!Number.isFinite(request.size) || request.size <= 0) {
      this.violations.push("invalid_edge_size");
    }
    const edge = this.ensureEdge(request.position, request.size, request.collapsed);
    edge.size = request.size;
    edge.visible = true;
    edge.collapsed = request.collapsed;
    edge.headerPosition =
      request.headerPosition ??
      edge.headerPosition ??
      defaultHeaderPositionForEdge(request.position);
    this.commands.push({ kind: "configure-edge", request: { ...request } });
    return {
      position: edge.position,
      exists: true,
      groupId: edge.groupId,
      visible: edge.visible,
      collapsed: edge.collapsed,
      size: edge.size,
    };
  }

  private activate(panelInstanceId: string): boolean {
    const panel = this.panels.get(panelInstanceId);
    if (!panel?.metadata) return false;
    this.setActiveState(panel.id);
    this.commands.push({ kind: "activate", panelId: panel.id });
    return true;
  }

  private reveal(panelInstanceId: string): boolean {
    const panel = this.panels.get(panelInstanceId);
    if (!panel?.metadata) return false;
    this.setActiveState(panel.id);
    const group = this.groups.get(panel.groupId);
    if (group?.location.type === "edge") {
      const edge = this.edges.get(group.location.position);
      if (edge) {
        edge.visible = true;
        edge.collapsed = false;
      }
    }
    this.commands.push({ kind: "reveal", panelId: panel.id });
    return true;
  }

  private remapResource(from: string, to: string): number {
    let count = 0;
    for (const panel of this.panels.values()) {
      if (!panel.metadata) continue;
      let metadata: WorkbenchPanelMetadata | undefined;
      try {
        metadata = remappedMetadata(panel.metadata, from, to);
      } catch {
        this.violations.push("invalid_panel_metadata");
        continue;
      }
      if (!metadata) continue;
      panel.metadata = metadata;
      panel.params = { ...panel.params, metadata: cloneMetadata(metadata) };
      panel.component = componentForWorkbenchMetadata(metadata);
      this.commands.push({
        kind: "update-panel",
        panelId: panel.id,
        metadata,
        updateTitle: false,
      });
      count += 1;
    }
    return count;
  }

  private removePanels(panelInstanceIds: readonly string[]): void {
    const uniqueIds = new Set(panelInstanceIds);
    for (const panelId of uniqueIds) {
      const panel = this.panels.get(panelId);
      if (!panel?.metadata) this.rejectInvalidRemove(panelId);
      if (!canRemoveWorkbenchPanel(panel.metadata)) this.rejectInvalidRemove(panelId);
      const group = this.groups.get(panel.groupId);
      if (!group) this.rejectInvalidRemove(panelId);
      const index = group.panelIds.indexOf(panelId);
      if (index < 0) this.rejectInvalidRemove(panelId);
      const wasActive = panel.active;
      const deferUntilFinal = group.location.type === "grid" && group.panelIds.length === 1;
      group.panelIds.splice(index, 1);
      if (group.activePanelId === panelId) group.activePanelId = group.panelIds[0];
      this.panels.delete(panelId);
      this.commands.push({ kind: "remove", panelId, deferUntilFinal });
      if (group.panelIds.length === 0 && group.location.type === "grid") {
        this.deleteGroup(group.id);
      }
      if (wasActive) this.setReplacementActiveState(group.id);
    }
  }

  private rejectInvalidRemove(panelInstanceId: string): never {
    this.violations.push("invalid_remove_target");
    throw new WorkbenchLayoutError("layout_restore_failed", {
      reason: "invalid_remove_target",
      panelInstanceId,
    });
  }

  private ensureEdge(
    position: WorkbenchEdgePosition,
    size = DEFAULT_EDGE_SIZES[position] ?? 200,
    collapsed = false,
  ): ShadowEdge {
    const existing = this.edges.get(position);
    if (existing) return existing;
    const groupId = configuredEdgeId(position);
    if (this.groups.has(groupId)) {
      this.violations.push("duplicate_group");
    }
    const group: ShadowGroup = {
      id: groupId,
      location: { type: "edge", position },
      panelIds: [],
      activePanelId: undefined,
      active: false,
      headerPosition: defaultHeaderPositionForEdge(position),
    };
    const edge: ShadowEdge = {
      position,
      groupId,
      visible: true,
      collapsed,
      size,
      headerPosition: defaultHeaderPositionForEdge(position),
    };
    this.groups.set(groupId, group);
    this.groupOrder.push(groupId);
    this.edges.set(position, edge);
    this.commands.push({ kind: "add-edge", position, groupId, size, collapsed });
    return edge;
  }

  private findReplacementActivePanel(preferredGroupId?: string): string | undefined {
    const preferred = preferredGroupId ? this.groups.get(preferredGroupId) : undefined;
    if (preferred?.activePanelId && this.panels.has(preferred.activePanelId)) {
      return preferred.activePanelId;
    }
    for (const groupId of this.groupOrder) {
      const group = this.groups.get(groupId);
      if (group?.activePanelId && this.panels.has(group.activePanelId)) {
        return group.activePanelId;
      }
      const firstPanelId = group?.panelIds.find((panelId) => this.panels.has(panelId));
      if (firstPanelId) return firstPanelId;
    }
    return undefined;
  }

  private findReplacementActiveGroup(preferredGroupId?: string): string | undefined {
    if (preferredGroupId && this.groups.has(preferredGroupId)) return preferredGroupId;
    return this.groupOrder.find((groupId) => this.groups.has(groupId));
  }

  private setReplacementActiveState(preferredGroupId?: string): void {
    const panelId = this.findReplacementActivePanel(preferredGroupId);
    if (panelId) this.setActiveState(panelId);
    else this.setActiveGroupState(this.findReplacementActiveGroup(preferredGroupId));
  }

  private setActiveState(panelId: string | undefined): void {
    for (const panel of this.panels.values()) panel.active = false;
    for (const group of this.groups.values()) group.active = false;
    if (!panelId) return;
    const panel = this.panels.get(panelId);
    const group = panel ? this.groups.get(panel.groupId) : undefined;
    if (!panel || !group) return;
    panel.active = true;
    group.active = true;
    group.activePanelId = panel.id;
  }

  private setActiveGroupState(groupId: string | undefined): void {
    for (const panel of this.panels.values()) panel.active = false;
    for (const group of this.groups.values()) group.active = false;
    const group = groupId ? this.groups.get(groupId) : undefined;
    if (group) group.active = true;
  }

  private deleteGroup(groupId: string): void {
    const group = this.groups.get(groupId);
    if (group?.location.type === "edge") this.edges.delete(group.location.position);
    this.groups.delete(groupId);
    const index = this.groupOrder.indexOf(groupId);
    if (index >= 0) this.groupOrder.splice(index, 1);
  }

  private toPanelInfo(panel: ShadowPanel): WorkbenchPanelInfo | undefined {
    if (!panel.metadata) return undefined;
    const group = this.groups.get(panel.groupId);
    if (!group) return undefined;
    return {
      panelInstanceId: panel.id,
      groupId: panel.groupId,
      component: componentForWorkbenchMetadata(panel.metadata),
      title: panel.title,
      metadata: cloneMetadata(panel.metadata),
      active: panel.active,
      location: { ...group.location },
    };
  }

  private toGroupInfo(group: ShadowGroup): WorkbenchGroupInfo {
    const panelInstanceIds = group.panelIds.filter(
      (panelId) => this.panels.get(panelId)?.metadata !== undefined,
    );
    const activePanelInstanceId =
      group.activePanelId && this.panels.get(group.activePanelId)?.metadata !== undefined
        ? group.activePanelId
        : undefined;
    return {
      groupId: group.id,
      panelInstanceIds,
      ...(activePanelInstanceId ? { activePanelInstanceId } : {}),
      active: group.active,
      location: { ...group.location },
    };
  }

  private serialize(): SerializedDockview {
    const layout = structuredClone(this.baseLayout) as unknown as MutableSerializedLayout;
    const representedGroups = new Set<string>();
    const patchNode = (node: MutableSerializedNode): MutableSerializedNode | undefined => {
      if (node.type === "leaf") {
        const groupId = typeof node.data.id === "string" ? node.data.id : "";
        const group = this.groups.get(groupId);
        if (!group || group.location.type !== "grid") return undefined;
        representedGroups.add(groupId);
        return {
          ...node,
          data: {
            ...node.data,
            id: group.id,
            views: [...group.panelIds],
            activeView: group.activePanelId ?? "",
          },
        };
      }
      const children = node.data.flatMap((child) => {
        const patched = patchNode(child);
        return patched ? [patched] : [];
      });
      if (children.length === 0) return undefined;
      if (children.length === 1) return children[0];
      return { ...node, data: children };
    };

    const patchedRoot = patchNode(layout.grid.root);
    const missingLeaves = this.groupOrder.flatMap((groupId) => {
      const group = this.groups.get(groupId);
      if (!group || group.location.type !== "grid" || representedGroups.has(groupId)) return [];
      return [
        {
          type: "leaf" as const,
          data: {
            id: group.id,
            views: [...group.panelIds],
            activeView: group.activePanelId ?? "",
          },
        },
      ];
    });
    const roots = [...(patchedRoot ? [patchedRoot] : []), ...missingLeaves];
    layout.grid.root = roots.length === 1 ? roots[0] : { type: "branch", data: roots };

    const nextPanels: Record<string, UnknownRecord> = {};
    for (const panel of this.panels.values()) {
      const existing = layout.panels[panel.id] ?? {};
      nextPanels[panel.id] = {
        ...existing,
        id: panel.id,
        contentComponent: panel.component,
        title: panel.title,
        params: panel.metadata
          ? { ...panel.params, metadata: cloneMetadata(panel.metadata) }
          : { ...panel.params },
      };
    }
    layout.panels = nextPanels;

    const serializedEdges = layout.edgeGroups ?? {};
    for (const position of EDGE_POSITIONS) {
      const edge = this.edges.get(position);
      if (!edge) {
        delete serializedEdges[position];
        continue;
      }
      const group = this.groups.get(edge.groupId);
      if (!group) continue;
      const existing = serializedEdges[position] ?? {};
      const existingGroup = isRecord(existing.group) ? existing.group : {};
      serializedEdges[position] = {
        ...existing,
        size: edge.size,
        visible: edge.visible,
        collapsed: edge.collapsed,
        group: {
          ...existingGroup,
          id: group.id,
          views: [...group.panelIds],
          activeView: group.activePanelId ?? "",
          ...(edge.headerPosition ? { headerPosition: edge.headerPosition } : {}),
        },
      };
    }
    layout.edgeGroups = serializedEdges;
    const activeGroup = this.groupOrder
      .map((groupId) => this.groups.get(groupId))
      .find((group) => group?.active);
    if (activeGroup) layout.activeGroup = activeGroup.id;
    else delete layout.activeGroup;
    return layout as unknown as SerializedDockview;
  }

  private ensureSelectionConsistency(api: DockviewApi): void {
    for (const groupId of this.groupOrder) {
      const desiredGroup = this.groups.get(groupId);
      if (!desiredGroup) this.fail("missing_group");
      const liveGroup = this.requireFinalGroup(api, groupId);
      if (desiredGroup.activePanelId) {
        this.setFinalPanelActive(api, liveGroup, desiredGroup.activePanelId);
      }
    }

    const desiredActiveGroup = this.groupOrder
      .map((groupId) => this.groups.get(groupId))
      .find((group) => group?.active);
    if (!desiredActiveGroup) return;
    const liveActiveGroup = this.requireFinalGroup(api, desiredActiveGroup.id);
    if (desiredActiveGroup.activePanelId) {
      this.setFinalPanelActive(api, liveActiveGroup, desiredActiveGroup.activePanelId);
      return;
    }
    throwAsLayoutError("layout_restore_failed", { groupId: desiredActiveGroup.id }, () =>
      liveActiveGroup.api.setActive(),
    );
  }

  private requireFinalGroup(api: DockviewApi, groupId: string): IDockviewGroupPanel {
    const group = throwAsLayoutError("layout_restore_failed", { groupId }, () =>
      api.getGroup(groupId),
    );
    if (!group) throw new WorkbenchLayoutError("layout_restore_failed", { groupId });
    return group;
  }

  private setFinalPanelActive(
    api: DockviewApi,
    group: IDockviewGroupPanel,
    panelInstanceId: string,
  ): void {
    const details = { groupId: group.id, panelInstanceId };
    const panel = throwAsLayoutError("layout_restore_failed", details, () => {
      const target = api.getPanel(panelInstanceId);
      if (!target || target.group.id !== group.id || !group.panels.includes(target)) {
        throw new WorkbenchLayoutError("layout_restore_failed", details);
      }
      return target;
    });
    throwAsLayoutError("layout_restore_failed", details, () => panel.api.setActive());
  }

  private applyCommand(api: DockviewApi, command: BufferedCommand): void {
    switch (command.kind) {
      case "add-grid":
        api.addGroup({ id: command.groupId, direction: "right" } as AddGroupOptions);
        return;
      case "add-edge": {
        const group = api.addEdgeGroup(command.position, {
          id: command.groupId,
          initialSize: command.size,
          collapsed: command.collapsed,
        });
        group.setHeaderPosition(defaultHeaderPositionForEdge(command.position));
        return;
      }
      case "add-panel": {
        const panel = api.addPanel<WorkbenchPanelParams>({
          id: command.panelId,
          component: command.component,
          ...(command.metadata.role === "plugin" ? { renderer: "always" as const } : {}),
          title: command.title,
          params: { metadata: cloneMetadata(command.metadata) },
          position: {
            referenceGroup: command.groupId,
            ...(command.index === undefined ? {} : { index: command.index }),
          },
          inactive: true,
        } as AddPanelOptions<WorkbenchPanelParams>);
        if (command.metadata.role === "editor") {
          panel.api.setPinned(command.metadata.pinned ?? false);
        }
        return;
      }
      case "update-panel": {
        const panel = api.getPanel(command.panelId);
        if (!panel)
          throw new WorkbenchLayoutError("layout_restore_failed", {
            panelInstanceId: command.panelId,
          });
        updatePanelMetadata(panel, command.metadata);
        if (command.updateTitle && command.title !== undefined) panel.api.setTitle(command.title);
        if (command.pinned !== undefined) panel.api.setPinned(command.pinned);
        return;
      }
      case "move": {
        const panel = api.getPanel(command.panelId);
        const group = api.getGroup(command.groupId);
        if (!panel || !group)
          throw new WorkbenchLayoutError("layout_restore_failed", {
            panelInstanceId: command.panelId,
            groupId: command.groupId,
          });
        panel.api.moveTo({
          group: group as DockviewGroupPanel,
          ...(command.index === undefined ? {} : { index: command.index }),
          skipSetActive: !command.activate,
        });
        return;
      }
      case "configure-edge":
        configureEdgeLive(api, command.request);
        return;
      case "reveal": {
        const panel = api.getPanel(command.panelId);
        if (!panel)
          throw new WorkbenchLayoutError("layout_restore_failed", {
            panelInstanceId: command.panelId,
          });
        revealPanel(api, panel);
        return;
      }
      case "activate": {
        const panel = api.getPanel(command.panelId);
        if (!panel)
          throw new WorkbenchLayoutError("layout_restore_failed", {
            panelInstanceId: command.panelId,
          });
        panel.api.setActive();
        return;
      }
      case "remove": {
        const panel = api.getPanel(command.panelId);
        if (!panel)
          throw new WorkbenchLayoutError("layout_restore_failed", {
            panelInstanceId: command.panelId,
          });
        throwAsLayoutError("layout_restore_failed", { panelInstanceId: command.panelId }, () =>
          panel.api.close(),
        );
        return;
      }
    }
  }

  private uniqueId(): string {
    let id = generatedId();
    while (this.panels.has(id) || this.groups.has(id)) id = generatedId();
    return id;
  }

  private fail(reason: string): never {
    throw new WorkbenchLayoutError("layout_restore_failed", { reason });
  }
}
