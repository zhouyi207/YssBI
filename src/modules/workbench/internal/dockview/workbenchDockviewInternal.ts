import type {
  AddGroupOptions,
  AddPanelOptions,
  DockviewApi,
  DockviewGroupPanel,
  DockviewGroupPanelApi,
  IDockviewGroupPanel,
  IDockviewPanel,
  SerializedDockview,
} from "dockview-react";

import {
  canMoveWorkbenchPanel,
  canRemoveWorkbenchPanel,
  canSplitWorkbenchPanel,
  vetoInvalidWorkbenchActivityDrop,
} from "./workbenchActivityGroup";
import {
  WORKBENCH_EDGE_GROUP_IDS,
  WORKBENCH_EDGE_SIZES,
  WORKBENCH_HOME_EDGE,
} from "./workbenchDockviewDefaults";
import {
  componentForWorkbenchMetadata,
  isWorkbenchPanelMetadata,
  type EditorPanelMetadata,
  type WorkbenchComponentId,
  type WorkbenchPanelMetadata,
  type WorkbenchPanelParams,
} from "./workbenchPanelModel";
import type {
  ConfiguredWorkbenchEdgeState,
  ConfigureWorkbenchEdgeRequest,
  EnsureViewRequest,
  MoveWorkbenchPanelRequest,
  WorkbenchDockviewReadContract,
  WorkbenchDockviewControlContract,
  WorkbenchEdgePosition,
  WorkbenchEdgeState,
  WorkbenchEditorPanelInfo,
  WorkbenchGroupInfo,
  WorkbenchLayoutErrorCode,
  WorkbenchPanelCommitToken,
  WorkbenchPanelInfo,
} from "./workbenchTypes";
import { WorkbenchLayoutError } from "./workbenchTypes";

export interface WorkbenchDockviewTransaction {
  listPanels(): readonly WorkbenchPanelInfo[];
  remapResource(from: string, to: string): number;
  removePanels(panelInstanceIds: readonly string[]): void;
}

export interface WorkbenchLayoutTransaction {
  serialize(): SerializedDockview;
  getPanel(panelInstanceId: string): WorkbenchPanelInfo | undefined;
  getActivePanel(): WorkbenchPanelInfo | undefined;
  listPanels(): readonly WorkbenchPanelInfo[];
  listGroups(): readonly WorkbenchGroupInfo[];
  listGroupPanels(groupId: string): readonly WorkbenchPanelInfo[];
  ensureCentralGroup(): string;
  ensureView(request: EnsureViewRequest): WorkbenchPanelInfo;
  move(request: MoveWorkbenchPanelRequest): boolean;
  configureEdge(request: ConfigureWorkbenchEdgeRequest): ConfiguredWorkbenchEdgeState;
  activate(panelInstanceId: string): boolean;
  removePanels(panelInstanceIds: readonly string[]): void;
}

export interface WorkbenchDockviewInternal {
  bind(api: DockviewApi): void;
  unbind(api?: DockviewApi): void;
  beginHydration(): number;
  completeHydration(epoch?: number): void;
  invalidateHydration(): void;
  invalidatePendingOperations(): void;
  whenIdle(): Promise<void>;
  commitRemove(
    expected: readonly WorkbenchPanelCommitToken[],
    authorize?: () => boolean,
  ): Promise<"committed" | "stale">;
  installHydrationLayout<T>(
    epoch: number,
    operation: (transaction: WorkbenchLayoutTransaction) => T,
  ): T;
  runLayoutTransaction<T>(operation: (transaction: WorkbenchLayoutTransaction) => T): Promise<T>;
  runPublicationTransaction<T>(
    operation: (transaction: WorkbenchDockviewTransaction) => T | Promise<T>,
  ): Promise<T>;
}

type WorkbenchLocation = WorkbenchPanelInfo["location"];
type Disposable = { dispose(): void };
type UnknownRecord = Record<string, unknown>;

const EDGE_POSITIONS: readonly WorkbenchEdgePosition[] = ["top", "bottom", "left", "right"];
const DEFAULT_EDGE_IDS: Partial<Record<WorkbenchEdgePosition, string>> = WORKBENCH_EDGE_GROUP_IDS;
const DEFAULT_EDGE_SIZES: Partial<Record<WorkbenchEdgePosition, number>> = WORKBENCH_EDGE_SIZES;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function cloneMetadata(metadata: WorkbenchPanelMetadata): WorkbenchPanelMetadata {
  if (metadata.role === "editor") {
    return {
      role: "editor",
      resourceRef: metadata.resourceRef,
      resourceKind: metadata.resourceKind,
      ...(metadata.pinned === undefined ? {} : { pinned: metadata.pinned }),
      ...(metadata.sticky === undefined ? {} : { sticky: metadata.sticky }),
    };
  }
  if (metadata.role === "view") {
    return { role: "view", viewId: metadata.viewId };
  }
  const presentation =
    metadata.presentation.kind === "inspector"
      ? { kind: "inspector" as const }
      : metadata.presentation.kind === "plot"
        ? { kind: "plot" as const, chart: metadata.presentation.chart }
        : { kind: "report" as const, report: metadata.presentation.report };
  const source =
    metadata.source === null
      ? null
      : {
          graphPath: metadata.source.graphPath,
          port:
            metadata.source.port.kind === "declared"
              ? {
                  kind: "declared" as const,
                  nodeId: metadata.source.port.nodeId,
                  portKey: metadata.source.port.portKey,
                }
              : {
                  kind: "instance" as const,
                  nodeId: metadata.source.port.nodeId,
                  templateKey: metadata.source.port.templateKey,
                  instanceId: metadata.source.port.instanceId,
                },
        };
  return {
    role: "result",
    resultKey: metadata.resultKey,
    resultId: metadata.resultId,
    title: metadata.title,
    presentation,
    source,
  };
}

function readMetadata(panel: IDockviewPanel): WorkbenchPanelMetadata | undefined {
  const params = panel.params;
  if (!isRecord(params) || !isWorkbenchPanelMetadata(params.metadata)) return undefined;
  return cloneMetadata(params.metadata);
}

function panelParams(panel: IDockviewPanel): UnknownRecord {
  return isRecord(panel.params) ? panel.params : {};
}

function metadataEqual(left: WorkbenchPanelMetadata, right: WorkbenchPanelMetadata): boolean {
  return JSON.stringify(cloneMetadata(left)) === JSON.stringify(cloneMetadata(right));
}

function readLocation(group: IDockviewGroupPanel): WorkbenchLocation | undefined {
  const location = group.api.location;
  if (location.type === "grid") return { type: "grid" };
  if (location.type === "edge") {
    return { type: "edge", position: location.position };
  }
  return undefined;
}

function panelInfo(panel: IDockviewPanel | undefined): WorkbenchPanelInfo | undefined {
  if (!panel) return undefined;
  const metadata = readMetadata(panel);
  const location = readLocation(panel.group);
  if (!metadata || !location) return undefined;
  const visible = panel.api.isVisible;
  return {
    panelInstanceId: panel.id,
    groupId: panel.group.id,
    component: componentForWorkbenchMetadata(metadata),
    title: panel.title,
    metadata,
    active: panel.api.isActive,
    ...(typeof visible === "boolean" ? { visible } : {}),
    location,
  };
}

function isEditorPanelInfo(panel: WorkbenchPanelInfo): panel is WorkbenchEditorPanelInfo {
  return panel.metadata.role === "editor";
}

function groupInfo(api: DockviewApi, group: IDockviewGroupPanel): WorkbenchGroupInfo | undefined {
  const location = readLocation(group);
  if (!location) return undefined;
  const panelInstanceIds = group.panels
    .filter((panel) => readMetadata(panel) !== undefined)
    .map((panel) => panel.id);
  const activePanelInstanceId =
    group.activePanel && readMetadata(group.activePanel) !== undefined
      ? group.activePanel.id
      : undefined;
  return {
    groupId: group.id,
    panelInstanceIds,
    ...(activePanelInstanceId ? { activePanelInstanceId } : {}),
    active: api.activeGroup?.id === group.id,
    location,
  };
}

function listPanelInfo(api: DockviewApi): readonly WorkbenchPanelInfo[] {
  return api.panels.flatMap((panel) => {
    const info = panelInfo(panel);
    return info ? [info] : [];
  });
}

function listGroupInfo(api: DockviewApi): readonly WorkbenchGroupInfo[] {
  return api.groups.flatMap((group) => {
    const info = groupInfo(api, group);
    return info ? [info] : [];
  });
}

function edgeSize(
  api: DockviewApi,
  position: WorkbenchEdgePosition,
  group: DockviewGroupPanelApi,
): number {
  try {
    const serializedSize = api.toJSON().edgeGroups?.[position]?.size;
    if (typeof serializedSize === "number" && Number.isFinite(serializedSize)) {
      return serializedSize;
    }
  } catch {
    // Fall back to live geometry while Dockview is between layout passes.
  }
  return position === "left" || position === "right" ? group.width : group.height;
}

function readEdgeState(api: DockviewApi, position: WorkbenchEdgePosition): WorkbenchEdgeState {
  const group = api.getEdgeGroup(position);
  if (!group) {
    return {
      position,
      exists: false,
      visible: false,
      collapsed: false,
    };
  }
  return {
    position,
    exists: true,
    groupId: group.id,
    visible: api.isEdgeGroupVisible(position),
    collapsed: group.isCollapsed(),
    size: edgeSize(api, position, group),
  };
}

function configuredEdgeState(
  api: DockviewApi,
  position: WorkbenchEdgePosition,
): ConfiguredWorkbenchEdgeState {
  const state = readEdgeState(api, position);
  if (!state.exists || !state.groupId) {
    throw new WorkbenchLayoutError("layout_restore_failed", { position });
  }
  return { ...state, exists: true, groupId: state.groupId };
}

function throwAsLayoutError<T>(
  code: WorkbenchLayoutErrorCode,
  details: Readonly<Record<string, string>>,
  operation: () => T,
): T {
  try {
    return operation();
  } catch (error) {
    if (error instanceof WorkbenchLayoutError) throw error;
    throw new WorkbenchLayoutError(code, details);
  }
}

function requireValidMetadata(metadata: WorkbenchPanelMetadata): WorkbenchPanelMetadata {
  if (!isWorkbenchPanelMetadata(metadata)) {
    throw new WorkbenchLayoutError("invalid_panel_metadata");
  }
  return cloneMetadata(metadata);
}

function updatePanelMetadata(panel: IDockviewPanel, metadata: WorkbenchPanelMetadata): void {
  panel.api.updateParameters({
    ...panelParams(panel),
    metadata: cloneMetadata(metadata),
  });
}

function defaultHeaderPositionForEdge(
  position: WorkbenchEdgePosition,
): "top" | "bottom" | "left" | "right" {
  if (position === "bottom") return "bottom";
  if (position === "left") return "left";
  if (position === "right") return "right";
  return "top";
}

function setGroupSize(
  group: DockviewGroupPanelApi,
  position: WorkbenchEdgePosition,
  size: number,
): void {
  if (position === "left" || position === "right") group.setSize({ width: size });
  else group.setSize({ height: size });
}

function validateEdgeSize(position: WorkbenchEdgePosition, size: number): void {
  if (!Number.isFinite(size) || size <= 0) {
    throw new WorkbenchLayoutError("layout_restore_failed", { position });
  }
}

function generatedId(): string {
  return crypto.randomUUID();
}

function configuredEdgeId(position: WorkbenchEdgePosition): string {
  return DEFAULT_EDGE_IDS[position] ?? generatedId();
}

function revealPanel(api: DockviewApi, panel: IDockviewPanel): void {
  panel.api.setActive();
  const location = readLocation(panel.group);
  if (location?.type !== "edge") return;
  api.setEdgeGroupVisible(location.position, true);
  api.getEdgeGroup(location.position)?.expand();
}

function ensureCentralGroupLive(api: DockviewApi): string {
  if (api.activeGroup && readLocation(api.activeGroup)?.type === "grid") {
    return api.activeGroup.id;
  }
  const existing = api.groups.find((group) => readLocation(group)?.type === "grid");
  if (existing) return existing.id;
  return api.addGroup().id;
}

function requireGridGroup(api: DockviewApi, groupId: string): IDockviewGroupPanel {
  const group = api.getGroup(groupId);
  if (!group || readLocation(group)?.type !== "grid") {
    throw new WorkbenchLayoutError("group_not_found", { groupId });
  }
  return group;
}

function requireGroup(api: DockviewApi, groupId: string): IDockviewGroupPanel {
  const group = api.getGroup(groupId);
  if (!group || !readLocation(group)) {
    throw new WorkbenchLayoutError("group_not_found", { groupId });
  }
  return group;
}

function ensureHomeEdgeLive(
  api: DockviewApi,
  position: WorkbenchEdgePosition,
): DockviewGroupPanelApi {
  let group = api.getEdgeGroup(position);
  if (!group) {
    group = api.addEdgeGroup(position, {
      id: configuredEdgeId(position),
      initialSize: DEFAULT_EDGE_SIZES[position] ?? 200,
      collapsed: false,
    });
  }
  group.setHeaderPosition(defaultHeaderPositionForEdge(position));
  return group;
}

function configureEdgeLive(
  api: DockviewApi,
  request: ConfigureWorkbenchEdgeRequest,
): ConfiguredWorkbenchEdgeState {
  validateEdgeSize(request.position, request.size);
  let group = api.getEdgeGroup(request.position);
  if (!group) {
    group = api.addEdgeGroup(request.position, {
      id: configuredEdgeId(request.position),
      initialSize: request.size,
      collapsed: request.collapsed,
    });
  }
  api.setEdgeGroupVisible(request.position, true);
  setGroupSize(group, request.position, request.size);
  const headerPosition = request.headerPosition ?? defaultHeaderPositionForEdge(request.position);
  if (headerPosition) group.setHeaderPosition(headerPosition);
  if (request.collapsed) group.collapse();
  else group.expand();
  return configuredEdgeState(api, request.position);
}

function createPanelLive(
  api: DockviewApi,
  metadata: WorkbenchPanelMetadata,
  title: string,
  groupId: string,
  index?: number,
): IDockviewPanel {
  const canonical = requireValidMetadata(metadata);
  const options: AddPanelOptions<WorkbenchPanelParams> = {
    id: generatedId(),
    component: componentForWorkbenchMetadata(canonical),
    title,
    params: { metadata: canonical },
    position: {
      referenceGroup: groupId,
      ...(index === undefined ? {} : { index }),
    },
  };
  return api.addPanel(options);
}

function remappedMetadata(
  metadata: WorkbenchPanelMetadata,
  from: string,
  to: string,
): WorkbenchPanelMetadata | undefined {
  if (from === to) return undefined;
  if (metadata.role === "editor" && metadata.resourceRef === from) {
    return requireValidMetadata({ ...metadata, resourceRef: to });
  }
  if (metadata.role === "result" && metadata.source?.graphPath === from) {
    return requireValidMetadata({
      ...metadata,
      source: { ...metadata.source, graphPath: to },
    });
  }
  return undefined;
}

function remapLiveResources(api: DockviewApi, from: string, to: string): number {
  const updates: { panel: IDockviewPanel; metadata: WorkbenchPanelMetadata }[] = [];
  for (const panel of api.panels) {
    const metadata = readMetadata(panel);
    if (!metadata) continue;
    const remapped = remappedMetadata(metadata, from, to);
    if (remapped) updates.push({ panel, metadata: remapped });
  }
  updates.forEach(({ panel, metadata }) => updatePanelMetadata(panel, metadata));
  return updates.length;
}

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

class ShadowWorkbenchModel {
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
        size: edgeSize(api, position, group),
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

function isPromiseLike(value: unknown): value is PromiseLike<unknown> {
  return isRecord(value) && typeof value.then === "function";
}

export function createWorkbenchDockviewRuntime(): {
  readonly read: WorkbenchDockviewReadContract;
  readonly control: WorkbenchDockviewControlContract;
  readonly internal: WorkbenchDockviewInternal;
} {
  let api: DockviewApi | undefined;
  let bindingGeneration = 0;
  let operationGeneration = 0;
  let hydrated = false;
  let hydrationEpoch = 0;
  let revision = 0;
  let snapshot: Readonly<{ revision: number; ready: boolean; hydrated: boolean }> = Object.freeze({
    revision,
    ready: false,
    hydrated: false,
  });
  let draining = false;
  let listenerDeferralDepth = 0;
  let deferredNotification = false;
  const listeners = new Set<() => void>();
  const hydrationWaiters = new Set<(result: { readonly status: "hydrated" | "unbound" }) => void>();
  const idleWaiters = new Set<() => void>();
  const rootDisposables: Disposable[] = [];
  const edgeDisposables = new Map<WorkbenchEdgePosition, Disposable>();
  const panelDisposables = new Map<string, Disposable>();
  type PendingOperation = {
    readonly operationGeneration: number;
    readonly bindingGeneration?: number;
    run(boundApi: DockviewApi): Promise<void>;
    reject(error: unknown): void;
  };
  const queue: PendingOperation[] = [];

  const notifyListeners = (): void => {
    for (const listener of [...listeners]) {
      try {
        listener();
      } catch {
        // An observer cannot interrupt Dockview's authoritative mutation stream.
      }
    }
  };

  const publish = (): void => {
    revision += 1;
    snapshot = Object.freeze({ revision, ready: api !== undefined, hydrated });
    if (listenerDeferralDepth > 0) {
      deferredNotification = true;
      return;
    }
    notifyListeners();
  };

  const applyBufferedCommands = (hasCommands: boolean, operation: () => void): void => {
    const startingRevision = revision;
    let completed = false;
    listenerDeferralDepth += 1;
    try {
      operation();
      completed = true;
    } finally {
      listenerDeferralDepth -= 1;
      if (listenerDeferralDepth === 0) {
        if (completed && hasCommands && revision === startingRevision) {
          revision += 1;
          snapshot = Object.freeze({ revision, ready: api !== undefined, hydrated });
          deferredNotification = true;
        }
        if (deferredNotification) {
          deferredNotification = false;
          notifyListeners();
        }
      }
    }
  };

  const settleIdle = (): void => {
    if (draining || queue.length > 0) return;
    for (const resolve of idleWaiters) resolve();
    idleWaiters.clear();
  };

  const staleBindingError = (): WorkbenchLayoutError =>
    new WorkbenchLayoutError("dockview_not_ready", { reason: "stale_binding" });

  const rejectQueuedOperations = (predicate: (operation: PendingOperation) => boolean): void => {
    const retained: PendingOperation[] = [];
    for (const pending of queue) {
      if (predicate(pending)) pending.reject(staleBindingError());
      else retained.push(pending);
    }
    queue.splice(0, queue.length, ...retained);
    settleIdle();
  };

  const rebindEdgeListeners = (): void => {
    for (const disposable of edgeDisposables.values()) disposable.dispose();
    edgeDisposables.clear();
    if (!api) return;
    for (const position of EDGE_POSITIONS) {
      const edge = api.getEdgeGroup(position);
      if (!edge) continue;
      edgeDisposables.set(
        position,
        edge.onDidCollapsedChange(() => publish()),
      );
    }
  };

  const disposeSubscriptions = (): void => {
    rootDisposables.splice(0).forEach((disposable) => disposable.dispose());
    for (const disposable of edgeDisposables.values()) disposable.dispose();
    edgeDisposables.clear();
    for (const disposable of panelDisposables.values()) disposable.dispose();
    panelDisposables.clear();
  };

  const rebindPanelListeners = (): void => {
    for (const disposable of panelDisposables.values()) disposable.dispose();
    panelDisposables.clear();
    if (!api) return;

    for (const panel of api.panels) {
      const disposables: Disposable[] = [];
      if (typeof panel.api.onDidVisibilityChange === "function") {
        disposables.push(panel.api.onDidVisibilityChange(() => publish()));
      }
      if (typeof panel.api.onDidGroupChange === "function") {
        disposables.push(panel.api.onDidGroupChange(() => publish()));
      }
      if (disposables.length > 0) {
        panelDisposables.set(panel.id, {
          dispose: () => disposables.forEach((disposable) => disposable.dispose()),
        });
      }
    }
  };

  const drain = async (): Promise<void> => {
    if (draining || !api || !hydrated) return;
    draining = true;
    try {
      while (api && hydrated && queue.length > 0) {
        const next = queue.shift();
        if (next) await next.run(api);
      }
    } finally {
      draining = false;
      settleIdle();
      if (api && hydrated && queue.length > 0) void drain();
    }
  };

  type MutationContext = Readonly<{
    bindingGeneration: number;
    operationGeneration: number;
    hydrationEpoch: number;
  }>;

  const captureMutationContext = (): MutationContext => ({
    bindingGeneration,
    operationGeneration,
    hydrationEpoch,
  });

  const assertBindingContext = (boundApi: DockviewApi, expected: MutationContext): void => {
    if (
      api !== boundApi ||
      bindingGeneration !== expected.bindingGeneration ||
      operationGeneration !== expected.operationGeneration
    ) {
      throw staleBindingError();
    }
  };

  const assertMutationContext = (boundApi: DockviewApi, expected: MutationContext): void => {
    assertBindingContext(boundApi, expected);
    if (!hydrated || hydrationEpoch !== expected.hydrationEpoch) {
      throw new WorkbenchLayoutError("dockview_not_ready", { reason: "stale_hydration" });
    }
  };

  const assertHydrationLayoutContext = (
    boundApi: DockviewApi,
    expected: MutationContext,
    epoch: number,
  ): void => {
    assertBindingContext(boundApi, expected);
    if (hydrated || hydrationEpoch !== epoch || expected.hydrationEpoch !== epoch) {
      throw new WorkbenchLayoutError("dockview_not_ready", { reason: "stale_hydration" });
    }
  };

  const enqueue = <T>(operation: (boundApi: DockviewApi) => T | Promise<T>): Promise<T> => {
    const queuedOperationGeneration = operationGeneration;
    const queuedBindingGeneration = api === undefined ? undefined : bindingGeneration;
    return new Promise<T>((resolve, rejectPromise) => {
      let settled = false;
      const settleReject = (error: unknown): void => {
        if (settled) return;
        settled = true;
        rejectPromise(error);
      };
      queue.push({
        operationGeneration: queuedOperationGeneration,
        ...(queuedBindingGeneration === undefined
          ? {}
          : { bindingGeneration: queuedBindingGeneration }),
        reject: settleReject,
        async run(boundApi) {
          if (settled) return;
          try {
            if (
              operationGeneration !== queuedOperationGeneration ||
              (queuedBindingGeneration !== undefined &&
                (bindingGeneration !== queuedBindingGeneration || api !== boundApi))
            ) {
              throw staleBindingError();
            }
            const result = await operation(boundApi);
            if (settled) return;
            settled = true;
            resolve(result);
          } catch (error) {
            settleReject(error);
          }
        },
      });
      void drain();
    });
  };

  const runtime: WorkbenchDockviewReadContract & WorkbenchDockviewControlContract = {
    get isReady() {
      return api !== undefined;
    },
    get isHydrated() {
      return hydrated;
    },
    whenHydrated: () =>
      hydrated
        ? Promise.resolve({ status: "hydrated" as const })
        : new Promise((resolve) => hydrationWaiters.add(resolve)),
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    getSnapshot: () => snapshot,
    getPanel: (panelInstanceId) => panelInfo(api?.getPanel(panelInstanceId)),
    getActivePanel: () => panelInfo(api?.activePanel),
    getActiveEditorPanel: () => {
      const active = panelInfo(api?.activePanel);
      return active && isEditorPanelInfo(active) ? active : undefined;
    },
    getActiveEditorPanelInGroup: (groupId) => {
      const group = api?.getGroup(groupId);
      const active = panelInfo(group?.activePanel);
      return active && isEditorPanelInfo(active) ? active : undefined;
    },
    listPanels: () => (api ? listPanelInfo(api) : []),
    listGroups: () => (api ? listGroupInfo(api) : []),
    listGroupPanels: (groupId) => {
      const group = api?.getGroup(groupId);
      if (!group) return [];
      return group.panels.flatMap((panel) => {
        const info = panelInfo(panel);
        return info ? [info] : [];
      });
    },
    listEditorPanelsInGroup: (groupId) => {
      const group = api?.getGroup(groupId);
      if (!group) return [];
      return group.panels.flatMap((panel) => {
        const info = panelInfo(panel);
        return info && isEditorPanelInfo(info) ? [info] : [];
      });
    },
    findEditorPanelsByResource: (resourceRef) =>
      api
        ? listPanelInfo(api).filter(
            (panel): panel is WorkbenchEditorPanelInfo =>
              isEditorPanelInfo(panel) && panel.metadata.resourceRef === resourceRef,
          )
        : [],
    getEdgeState: (position) =>
      api
        ? readEdgeState(api, position)
        : { position, exists: false, visible: false, collapsed: false },
    ensureCentralGroup: () =>
      enqueue((boundApi) =>
        throwAsLayoutError("panel_open_failed", {}, () => ensureCentralGroupLive(boundApi)),
      ),
    openEditor: (request) =>
      enqueue((boundApi) =>
        throwAsLayoutError(
          "panel_open_failed",
          request.targetGroupId ? { groupId: request.targetGroupId } : {},
          () => {
            const metadata = requireValidMetadata({
              role: "editor",
              resourceRef: request.resourceRef,
              resourceKind: request.resourceKind,
              pinned: request.pinned,
              ...(request.sticky === undefined ? {} : { sticky: request.sticky }),
            });
            if (request.mode === "reuse-resource") {
              const existing = boundApi.panels.find((candidate) => {
                const candidateMetadata = readMetadata(candidate);
                return (
                  candidateMetadata?.role === "editor" &&
                  candidateMetadata.resourceRef === request.resourceRef
                );
              });
              if (existing) {
                const existingMetadata = readMetadata(existing);
                const requestedComponent = componentForWorkbenchMetadata(metadata);
                if (
                  existingMetadata?.role !== "editor" ||
                  existingMetadata.resourceKind !== request.resourceKind ||
                  existing.api.component !== requestedComponent
                ) {
                  throw new WorkbenchLayoutError("panel_open_failed", {
                    panelInstanceId: existing.id,
                  });
                }
                updatePanelMetadata(existing, metadata);
                if (existing.title !== request.title) existing.api.setTitle(request.title);
                existing.api.setPinned(request.pinned);
                revealPanel(boundApi, existing);
                const info = panelInfo(existing);
                if (info) return info;
              }
            }
            const groupId = request.targetGroupId
              ? requireGridGroup(boundApi, request.targetGroupId).id
              : ensureCentralGroupLive(boundApi);
            const panel = createPanelLive(
              boundApi,
              metadata,
              request.title,
              groupId,
              request.index,
            );
            panel.api.setPinned(request.pinned);
            const info = panelInfo(panel);
            if (!info) throw new WorkbenchLayoutError("invalid_panel_metadata");
            return info;
          },
        ),
      ),
    setEditorPinned: (panelInstanceId, pinned) =>
      enqueue((boundApi) =>
        throwAsLayoutError("panel_open_failed", { panelInstanceId }, () => {
          const panel = boundApi.getPanel(panelInstanceId);
          const metadata = panel ? readMetadata(panel) : undefined;
          if (!panel || metadata?.role !== "editor") return false;
          const next: EditorPanelMetadata = { ...metadata, pinned };
          updatePanelMetadata(panel, next);
          panel.api.setPinned(pinned);
          return true;
        }),
      ),
    ensureView: (request) =>
      enqueue((boundApi) =>
        throwAsLayoutError("panel_open_failed", { viewId: request.viewId }, () => {
          const existing = boundApi.panels.find((candidate) => {
            const metadata = readMetadata(candidate);
            return metadata?.role === "view" && metadata.viewId === request.viewId;
          });
          if (existing) {
            if (existing.title !== request.title) existing.api.setTitle(request.title);
            revealPanel(boundApi, existing);
            const info = panelInfo(existing);
            if (info) return info;
          }
          const position = WORKBENCH_HOME_EDGE[request.viewId];
          const group = ensureHomeEdgeLive(boundApi, position);
          const panel = createPanelLive(
            boundApi,
            { role: "view", viewId: request.viewId },
            request.title,
            group.id,
          );
          revealPanel(boundApi, panel);
          rebindEdgeListeners();
          const info = panelInfo(panel);
          if (!info) throw new WorkbenchLayoutError("invalid_panel_metadata");
          return info;
        }),
      ),
    upsertResult: (request) =>
      enqueue((boundApi) =>
        throwAsLayoutError("panel_open_failed", { resultKey: request.resultKey }, () => {
          const metadata = requireValidMetadata({ role: "result", ...request });
          const existing = boundApi.panels.find((candidate) => {
            const candidateMetadata = readMetadata(candidate);
            return (
              candidateMetadata?.role === "result" &&
              candidateMetadata.resultKey === request.resultKey
            );
          });
          if (existing) {
            updatePanelMetadata(existing, metadata);
            if (existing.title !== request.title) existing.api.setTitle(request.title);
            revealPanel(boundApi, existing);
            const info = panelInfo(existing);
            if (info) return info;
          }
          const position = WORKBENCH_HOME_EDGE.result;
          const group = ensureHomeEdgeLive(boundApi, position);
          const panel = createPanelLive(boundApi, metadata, request.title, group.id);
          revealPanel(boundApi, panel);
          rebindEdgeListeners();
          const info = panelInfo(panel);
          if (!info) throw new WorkbenchLayoutError("invalid_panel_metadata");
          return info;
        }),
      ),
    activate: (panelInstanceId) =>
      enqueue((boundApi) =>
        throwAsLayoutError("panel_open_failed", { panelInstanceId }, () => {
          const panel = boundApi.getPanel(panelInstanceId);
          if (!panel || !readMetadata(panel)) return false;
          panel.api.setActive();
          return true;
        }),
      ),
    reveal: (panelInstanceId) =>
      enqueue((boundApi) =>
        throwAsLayoutError("panel_open_failed", { panelInstanceId }, () => {
          const panel = boundApi.getPanel(panelInstanceId);
          if (!panel || !readMetadata(panel)) return false;
          revealPanel(boundApi, panel);
          return true;
        }),
      ),
    move: (request) =>
      enqueue((boundApi) =>
        throwAsLayoutError("group_not_found", { groupId: request.groupId }, () => {
          const panel = boundApi.getPanel(request.panelInstanceId);
          const metadata = panel ? readMetadata(panel) : undefined;
          if (!panel || !metadata) return false;
          const target = requireGroup(boundApi, request.groupId);
          const targetLocation = readLocation(target);
          if (!targetLocation) return false;
          const targetPosition =
            targetLocation.type === "edge" ? targetLocation.position : targetLocation.type;
          if (!canMoveWorkbenchPanel(metadata, target.id, targetPosition)) return false;
          const source = panel.group;
          const currentIndex = source.panels.indexOf(panel);
          const maximumIndex =
            source.id === target.id ? Math.max(0, target.panels.length - 1) : target.panels.length;
          const effectiveIndex =
            request.index === undefined
              ? source.id === target.id
                ? currentIndex
                : undefined
              : Math.min(Math.max(request.index, 0), maximumIndex);
          if (
            source.id === target.id &&
            (effectiveIndex === undefined || effectiveIndex === currentIndex)
          ) {
            if (request.activate !== false) panel.api.setActive();
            return true;
          }
          panel.api.moveTo({
            group: target as DockviewGroupPanel,
            ...(effectiveIndex === undefined ? {} : { index: effectiveIndex }),
            skipSetActive: request.activate === false,
          });
          return true;
        }),
      ),
    split: (request) =>
      enqueue((boundApi) =>
        throwAsLayoutError("group_not_found", { groupId: request.referenceGroupId }, () => {
          const panel = boundApi.getPanel(request.panelInstanceId);
          const metadata = panel ? readMetadata(panel) : undefined;
          if (!panel || !metadata) return false;
          const reference = requireGroup(boundApi, request.referenceGroupId);
          if (!canSplitWorkbenchPanel(metadata, reference.id)) return false;
          panel.api.moveTo({
            group: reference as DockviewGroupPanel,
            position: request.direction,
            skipSetActive: request.activate === false,
          });
          return true;
        }),
      ),
    configureEdge: (request) =>
      enqueue((boundApi) =>
        throwAsLayoutError("layout_restore_failed", { position: request.position }, () => {
          const state = configureEdgeLive(boundApi, request);
          rebindEdgeListeners();
          return state;
        }),
      ),
    setEdgeCollapsed: (position, collapsed) =>
      enqueue((boundApi) =>
        throwAsLayoutError("layout_restore_failed", { position }, () => {
          const edge = boundApi.getEdgeGroup(position);
          if (!edge) return false;
          if (collapsed) edge.collapse();
          else {
            boundApi.setEdgeGroupVisible(position, true);
            edge.expand();
          }
          return true;
        }),
      ),
    setEdgeSize: (position, size) =>
      enqueue((boundApi) =>
        throwAsLayoutError("layout_restore_failed", { position }, () => {
          validateEdgeSize(position, size);
          const edge = boundApi.getEdgeGroup(position);
          if (!edge) return false;
          setGroupSize(edge, position, size);
          return true;
        }),
      ),
    remapResource: (from, to) =>
      enqueue((boundApi) =>
        throwAsLayoutError("panel_open_failed", {}, () => remapLiveResources(boundApi, from, to)),
      ),
    serialize: () =>
      enqueue((boundApi) =>
        throwAsLayoutError("layout_restore_failed", {}, () => structuredClone(boundApi.toJSON())),
      ),
  };

  const internal: WorkbenchDockviewInternal = {
    bind(boundApi) {
      if (api === boundApi) {
        rebindEdgeListeners();
        void drain();
        return;
      }
      disposeSubscriptions();
      bindingGeneration += 1;
      api = boundApi;
      rootDisposables.push(
        boundApi.onDidLayoutChange(() => {
          rebindEdgeListeners();
          rebindPanelListeners();
          publish();
        }),
        boundApi.onDidLayoutFromJSON(() => {
          rebindEdgeListeners();
          rebindPanelListeners();
          publish();
        }),
        boundApi.onWillShowOverlay((event) => {
          vetoInvalidWorkbenchActivityDrop(event);
        }),
        boundApi.onWillDrop((event) => {
          vetoInvalidWorkbenchActivityDrop(event);
        }),
        boundApi.onDidActivePanelChange(() => publish()),
        boundApi.onDidActiveGroupChange(() => publish()),
      );
      rebindEdgeListeners();
      rebindPanelListeners();
      publish();
      void drain();
    },
    unbind(boundApi) {
      if (boundApi && boundApi !== api) return;
      const invalidatedBindingGeneration = bindingGeneration;
      disposeSubscriptions();
      bindingGeneration += 1;
      api = undefined;
      for (const resolve of hydrationWaiters) resolve({ status: "unbound" });
      hydrationWaiters.clear();
      rejectQueuedOperations(
        (pending) => pending.bindingGeneration === invalidatedBindingGeneration,
      );
      publish();
    },
    beginHydration() {
      hydrationEpoch += 1;
      if (hydrated) {
        hydrated = false;
        publish();
      }
      return hydrationEpoch;
    },
    completeHydration(epoch) {
      if (epoch !== undefined && epoch !== hydrationEpoch) return;
      if (!hydrated) {
        hydrated = true;
        for (const resolve of hydrationWaiters) resolve({ status: "hydrated" });
        hydrationWaiters.clear();
        publish();
      }
      void drain();
    },
    invalidateHydration() {
      hydrationEpoch += 1;
      if (hydrated) {
        hydrated = false;
        publish();
      }
    },
    invalidatePendingOperations() {
      operationGeneration += 1;
      rejectQueuedOperations(() => true);
    },
    whenIdle: () =>
      !draining && queue.length === 0
        ? Promise.resolve()
        : new Promise<void>((resolve) => idleWaiters.add(resolve)),
    commitRemove: (expected, authorize) =>
      enqueue((boundApi) => {
        if (authorize) {
          try {
            if (!authorize()) return "stale" as const;
          } catch {
            return "stale" as const;
          }
        }
        const panels: IDockviewPanel[] = [];
        const seen = new Set<string>();
        for (const token of expected) {
          const panel = throwAsLayoutError(
            "layout_restore_failed",
            { panelInstanceId: token.panelInstanceId },
            () => boundApi.getPanel(token.panelInstanceId),
          );
          const metadata = panel
            ? throwAsLayoutError(
                "layout_restore_failed",
                { panelInstanceId: token.panelInstanceId },
                () => readMetadata(panel),
              )
            : undefined;
          if (
            !panel ||
            !metadata ||
            panel.group.id !== token.groupId ||
            !metadataEqual(metadata, token.metadata) ||
            !canRemoveWorkbenchPanel(metadata)
          ) {
            return "stale" as const;
          }
          if (!seen.has(token.panelInstanceId)) {
            seen.add(token.panelInstanceId);
            panels.push(panel);
          }
        }
        panels.forEach((panel) =>
          throwAsLayoutError("layout_restore_failed", { panelInstanceId: panel.id }, () =>
            panel.api.close(),
          ),
        );
        return "committed" as const;
      }),
    installHydrationLayout: (epoch, operation) => {
      const boundApi = api;
      if (!boundApi) throw staleBindingError();
      const context = captureMutationContext();
      assertHydrationLayoutContext(boundApi, context, epoch);
      const shadow = throwAsLayoutError(
        "layout_restore_failed",
        {},
        () => new ShadowWorkbenchModel(boundApi),
      );
      const result = operation(shadow.layout);
      if (isPromiseLike(result)) {
        throw new WorkbenchLayoutError("layout_restore_failed", {
          reason: "async_layout_transaction",
        });
      }
      assertHydrationLayoutContext(boundApi, context, epoch);
      throwAsLayoutError("layout_restore_failed", {}, () => shadow.validate(boundApi));
      throwAsLayoutError("layout_restore_failed", {}, () =>
        applyBufferedCommands(shadow.hasBufferedCommands(), () => {
          shadow.apply(boundApi);
          rebindEdgeListeners();
        }),
      );
      return result;
    },
    runLayoutTransaction: (operation) =>
      enqueue((boundApi) => {
        const context = captureMutationContext();
        const shadow = throwAsLayoutError(
          "layout_restore_failed",
          {},
          () => new ShadowWorkbenchModel(boundApi),
        );
        const result = operation(shadow.layout);
        if (isPromiseLike(result)) {
          throw new WorkbenchLayoutError("layout_restore_failed", {
            reason: "async_layout_transaction",
          });
        }
        assertMutationContext(boundApi, context);
        throwAsLayoutError("layout_restore_failed", {}, () => shadow.validate(boundApi));
        throwAsLayoutError("layout_restore_failed", {}, () =>
          applyBufferedCommands(shadow.hasBufferedCommands(), () => {
            shadow.apply(boundApi);
            rebindEdgeListeners();
          }),
        );
        return result;
      }),
    runPublicationTransaction: (operation) =>
      enqueue(async (boundApi) => {
        const context = captureMutationContext();
        const shadow = throwAsLayoutError(
          "layout_restore_failed",
          {},
          () => new ShadowWorkbenchModel(boundApi),
        );
        const result = await operation(shadow.publication);
        assertMutationContext(boundApi, context);
        throwAsLayoutError("layout_restore_failed", {}, () => shadow.validate(boundApi));
        throwAsLayoutError("layout_restore_failed", {}, () =>
          applyBufferedCommands(shadow.hasBufferedCommands(), () => {
            shadow.apply(boundApi);
            rebindEdgeListeners();
          }),
        );
        return result;
      }),
  };

  return { read: runtime, control: runtime, internal };
}

export const workbenchDockviewRuntime = createWorkbenchDockviewRuntime();

export const workbenchDockviewInternal = workbenchDockviewRuntime.internal;
