import type {
  AddPanelOptions,
  DockviewApi,
  DockviewGroupPanelApi,
  IDockviewGroupPanel,
  IDockviewPanel,
  SerializedDockview,
} from "dockview-react";

import { WORKBENCH_EDGE_GROUP_IDS, WORKBENCH_EDGE_SIZES } from "./workbenchDockviewDefaults";
import {
  componentForWorkbenchMetadata,
  isWorkbenchPanelMetadata,
  type WorkbenchPanelMetadata,
  type WorkbenchPanelParams,
} from "./workbenchPanelModel";
import type {
  ConfiguredWorkbenchEdgeState,
  ConfigureWorkbenchEdgeRequest,
  WorkbenchEdgePosition,
  WorkbenchEdgeState,
  WorkbenchEditorPanelInfo,
  WorkbenchGroupInfo,
  WorkbenchLayoutErrorCode,
  WorkbenchPanelInfo,
} from "./workbenchTypes";
import { WorkbenchLayoutError } from "./workbenchTypes";

export type WorkbenchLocation = WorkbenchPanelInfo["location"];
export type Disposable = { dispose(): void };
export type UnknownRecord = Record<string, unknown>;

export const EDGE_POSITIONS: readonly WorkbenchEdgePosition[] = ["top", "bottom", "left", "right"];
export const DEFAULT_EDGE_IDS: Partial<Record<WorkbenchEdgePosition, string>> =
  WORKBENCH_EDGE_GROUP_IDS;
export const DEFAULT_EDGE_SIZES: Partial<Record<WorkbenchEdgePosition, number>> =
  WORKBENCH_EDGE_SIZES;

export function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function cloneMetadata(metadata: WorkbenchPanelMetadata): WorkbenchPanelMetadata {
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

export function readMetadata(panel: IDockviewPanel): WorkbenchPanelMetadata | undefined {
  const params = panel.params;
  if (!isRecord(params) || !isWorkbenchPanelMetadata(params.metadata)) return undefined;
  return cloneMetadata(params.metadata);
}

export function panelParams(panel: IDockviewPanel): UnknownRecord {
  return isRecord(panel.params) ? panel.params : {};
}

export function metadataEqual(
  left: WorkbenchPanelMetadata,
  right: WorkbenchPanelMetadata,
): boolean {
  return JSON.stringify(cloneMetadata(left)) === JSON.stringify(cloneMetadata(right));
}

export function readLocation(group: IDockviewGroupPanel): WorkbenchLocation | undefined {
  const location = group.api.location;
  if (location.type === "grid") return { type: "grid" };
  if (location.type === "edge") {
    return { type: "edge", position: location.position };
  }
  return undefined;
}

export function panelInfo(panel: IDockviewPanel | undefined): WorkbenchPanelInfo | undefined {
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

export function isEditorPanelInfo(panel: WorkbenchPanelInfo): panel is WorkbenchEditorPanelInfo {
  return panel.metadata.role === "editor";
}

export function groupInfo(
  api: DockviewApi,
  group: IDockviewGroupPanel,
): WorkbenchGroupInfo | undefined {
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

export function listPanelInfo(api: DockviewApi): readonly WorkbenchPanelInfo[] {
  return api.panels.flatMap((panel) => {
    const info = panelInfo(panel);
    return info ? [info] : [];
  });
}

export function listGroupInfo(api: DockviewApi): readonly WorkbenchGroupInfo[] {
  return api.groups.flatMap((group) => {
    const info = groupInfo(api, group);
    return info ? [info] : [];
  });
}

export function edgeSize(
  api: DockviewApi,
  position: WorkbenchEdgePosition,
  group: DockviewGroupPanelApi,
  layout?: SerializedDockview,
): number {
  try {
    const serializedSize = (layout ?? api.toJSON()).edgeGroups?.[position]?.size;
    if (typeof serializedSize === "number" && Number.isFinite(serializedSize)) {
      return serializedSize;
    }
  } catch {
    // Fall back to live geometry while Dockview is between layout passes.
  }
  return position === "left" || position === "right" ? group.width : group.height;
}

export function readEdgeState(
  api: DockviewApi,
  position: WorkbenchEdgePosition,
): WorkbenchEdgeState {
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
  };
}

export function configuredEdgeState(
  api: DockviewApi,
  position: WorkbenchEdgePosition,
): ConfiguredWorkbenchEdgeState {
  const state = readEdgeState(api, position);
  if (!state.exists || !state.groupId) {
    throw new WorkbenchLayoutError("layout_restore_failed", { position });
  }
  return {
    ...state,
    exists: true,
    groupId: state.groupId,
    size: edgeSize(api, position, api.getEdgeGroup(position)!),
  };
}

export function throwAsLayoutError<T>(
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

export function requireValidMetadata(metadata: WorkbenchPanelMetadata): WorkbenchPanelMetadata {
  if (!isWorkbenchPanelMetadata(metadata)) {
    throw new WorkbenchLayoutError("invalid_panel_metadata");
  }
  return cloneMetadata(metadata);
}

export function updatePanelMetadata(panel: IDockviewPanel, metadata: WorkbenchPanelMetadata): void {
  panel.api.updateParameters({
    ...panelParams(panel),
    metadata: cloneMetadata(metadata),
  });
}

export function defaultHeaderPositionForEdge(
  position: WorkbenchEdgePosition,
): "top" | "bottom" | "left" | "right" {
  if (position === "bottom") return "bottom";
  if (position === "left") return "left";
  if (position === "right") return "right";
  return "top";
}

export function setGroupSize(
  group: DockviewGroupPanelApi,
  position: WorkbenchEdgePosition,
  size: number,
): void {
  if (position === "left" || position === "right") group.setSize({ width: size });
  else group.setSize({ height: size });
}

export function validateEdgeSize(position: WorkbenchEdgePosition, size: number): void {
  if (!Number.isFinite(size) || size <= 0) {
    throw new WorkbenchLayoutError("layout_restore_failed", { position });
  }
}

export function generatedId(): string {
  return crypto.randomUUID();
}

export function configuredEdgeId(position: WorkbenchEdgePosition): string {
  return DEFAULT_EDGE_IDS[position] ?? generatedId();
}

export function revealPanel(api: DockviewApi, panel: IDockviewPanel): void {
  panel.api.setActive();
  const location = readLocation(panel.group);
  if (location?.type !== "edge") return;
  api.setEdgeGroupVisible(location.position, true);
  api.getEdgeGroup(location.position)?.expand();
}

export function ensureCentralGroupLive(api: DockviewApi): string {
  if (api.activeGroup && readLocation(api.activeGroup)?.type === "grid") {
    return api.activeGroup.id;
  }
  const existing = api.groups.find((group) => readLocation(group)?.type === "grid");
  if (existing) return existing.id;
  return api.addGroup().id;
}

export function requireGridGroup(api: DockviewApi, groupId: string): IDockviewGroupPanel {
  const group = api.getGroup(groupId);
  if (!group || readLocation(group)?.type !== "grid") {
    throw new WorkbenchLayoutError("group_not_found", { groupId });
  }
  return group;
}

export function requireGroup(api: DockviewApi, groupId: string): IDockviewGroupPanel {
  const group = api.getGroup(groupId);
  if (!group || !readLocation(group)) {
    throw new WorkbenchLayoutError("group_not_found", { groupId });
  }
  return group;
}

export function ensureHomeEdgeLive(
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

export function configureEdgeLive(
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

export function createPanelLive(
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

export function remappedMetadata(
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

export function remapLiveResources(api: DockviewApi, from: string, to: string): number {
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
