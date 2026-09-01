import type {
  AddGroupOptions,
  AddPanelOptions,
  DockviewApi,
  DockviewGroupPanel,
  DockviewGroupPanelApi,
  EdgeGroupPosition,
  IDockviewPanel,
  SerializedDockview,
} from "dockview-react";
import { describe, expect, it, type Mock, vi } from "vitest";

import * as publicDockview from "./index";
import {
  createWorkbenchDockviewRuntime,
  type WorkbenchDockviewInternal,
} from "./workbenchDockviewInternal";
import type { WorkbenchDockviewControl } from "./workbenchControl";
import type { WorkbenchDockviewRead } from "./workbenchRead";
import { WorkbenchLayoutError } from "./workbenchTypes";

type WorkbenchLocation =
  | { readonly type: "grid" }
  | { readonly type: "edge"; readonly position: EdgeGroupPosition };

type Disposable = { dispose(): void };

function createDockviewHarness(): {
  readonly port: WorkbenchDockviewRead & WorkbenchDockviewControl;
  readonly internal: WorkbenchDockviewInternal;
} {
  const runtime = createWorkbenchDockviewRuntime();
  const port = new Proxy({} as Record<PropertyKey, unknown>, {
    get(_target, property) {
      if (property in runtime.read) return Reflect.get(runtime.read, property);
      return Reflect.get(runtime.control, property);
    },
  }) as unknown as WorkbenchDockviewRead & WorkbenchDockviewControl;
  return { port, internal: runtime.internal };
}

type FakeMoveRequest = {
  group?: DockviewGroupPanel;
  position?: "top" | "bottom" | "left" | "right" | "center";
  index?: number;
  skipSetActive?: boolean;
};

function event<T>() {
  const listeners = new Set<(value: T) => void>();
  const subscribe = vi.fn((listener: (value: T) => void): Disposable => {
    listeners.add(listener);
    return { dispose: () => listeners.delete(listener) };
  });
  const emit = vi.fn((value: T): void => {
    [...listeners].forEach((listener) => listener(value));
  });
  return { emit, subscribe };
}

interface FakeGroup {
  readonly id: string;
  readonly location: WorkbenchLocation;
  readonly panels: IDockviewPanel[];
  readonly recentPanelIds: string[];
  activePanel: IDockviewPanel | undefined;
  headerPosition: "top" | "bottom" | "left" | "right";
  width: number;
  height: number;
  readonly api: DockviewGroupPanelApi;
  readonly dockviewGroup: DockviewGroupPanel;
  readonly collapsedChange: ReturnType<typeof event<{ isCollapsed: boolean }>>;
  collapsed: boolean;
}

interface FakePanelRecord {
  readonly panel: IDockviewPanel;
  readonly moveTo: Mock<(request: FakeMoveRequest) => void>;
  readonly close: Mock<() => void>;
  readonly setPinned: Mock<(pinned: boolean) => void>;
  params: Record<string, unknown>;
  title: string | undefined;
  group: FakeGroup;
  active: boolean;
  pinned: boolean;
}

interface FakeEdge {
  readonly group: FakeGroup;
  readonly addPanel: Mock<(options: AddPanelOptions<Record<string, unknown>>) => void>;
  readonly expand: Mock<() => void>;
}

class FakeWorkbenchDockview {
  readonly panels: IDockviewPanel[] = [];
  readonly groups: DockviewGroupPanel[] = [];
  readonly layoutChange = event<void>();
  readonly layoutFromJson = event<void>();
  readonly activeGroupChange = event<DockviewGroupPanel | undefined>();
  readonly activePanelChange = event<unknown>();
  readonly willDrop = event<unknown>();
  readonly willShowOverlay = event<unknown>();
  readonly addEdgeGroup = vi.fn(
    (
      position: EdgeGroupPosition,
      options: { id: string; initialSize?: number; collapsed?: boolean },
    ) => this.createEdge(position, options),
  );
  readonly getEdgeGroup = vi.fn(
    (position: EdgeGroupPosition) => this.edges.get(position)?.group.api,
  );
  readonly setEdgeGroupVisible = vi.fn((position: EdgeGroupPosition, visible: boolean) => {
    const edge = this.edges.get(position);
    if (edge) this.edgeVisibility.set(position, visible);
  });
  readonly fromJSON = vi.fn((_layout: SerializedDockview) => undefined);
  readonly toJSON = vi.fn(() => this.layout());
  readonly api: DockviewApi;

  private readonly groupRecords = new Map<string, FakeGroup>();
  private readonly panelRecords = new Map<string, FakePanelRecord>();
  private readonly edges = new Map<EdgeGroupPosition, FakeEdge>();
  private readonly edgeVisibility = new Map<EdgeGroupPosition, boolean>();
  private activePanelRecord: FakePanelRecord | undefined;
  private activeGroupRecord: FakeGroup | undefined;
  private nextGroupId = 1;

  constructor() {
    this.addGridGroup("grid-main");
    const self = this;
    this.api = {
      get panels() {
        return self.panels;
      },
      get groups() {
        return self.groups;
      },
      get activePanel() {
        return self.activePanelRecord?.panel;
      },
      get activeGroup() {
        return self.activeGroupRecord?.dockviewGroup;
      },
      onDidLayoutChange: this.layoutChange.subscribe,
      onDidLayoutFromJSON: this.layoutFromJson.subscribe,
      onDidActiveGroupChange: this.activeGroupChange.subscribe,
      onDidActivePanelChange: this.activePanelChange.subscribe,
      onWillDrop: this.willDrop.subscribe,
      onWillShowOverlay: this.willShowOverlay.subscribe,
      getPanel: (id: string) => self.panelRecords.get(id)?.panel,
      getGroup: (id: string) => self.groupRecords.get(id)?.dockviewGroup,
      addPanel: (options: AddPanelOptions<Record<string, unknown>>) => self.addPanel(options),
      addGroup: (options?: AddGroupOptions) =>
        self.addGridGroup(
          options?.id ?? `grid-${self.nextGroupId++}`,
          options?.skipSetActive !== true,
        ).dockviewGroup,
      addEdgeGroup: this.addEdgeGroup,
      getEdgeGroup: this.getEdgeGroup,
      setEdgeGroupVisible: this.setEdgeGroupVisible,
      isEdgeGroupVisible: (position: EdgeGroupPosition) =>
        self.edgeVisibility.get(position) ?? false,
      toJSON: this.toJSON,
      fromJSON: this.fromJSON,
    } as unknown as DockviewApi;
  }

  addGridGroup(id: string, activate = true): FakeGroup {
    const existing = this.groupRecords.get(id);
    if (existing) {
      if (activate) this.setActiveGroup(existing);
      return existing;
    }
    const group = this.createGroup(id, { type: "grid" });
    if (activate) this.setActiveGroup(group);
    return group;
  }

  edge(position: EdgeGroupPosition): FakeEdge {
    const edge = this.edges.get(position);
    if (!edge) throw new Error(`missing fake edge ${position}`);
    return edge;
  }

  panel(panelInstanceId: string): FakePanelRecord {
    const panel = this.panelRecords.get(panelInstanceId);
    if (!panel) throw new Error(`missing fake panel ${panelInstanceId}`);
    return panel;
  }

  movePanelToEdge(panelInstanceId: string, position: EdgeGroupPosition, collapsed: boolean): void {
    if (!this.edges.has(position)) {
      this.createEdge(position, { id: `edge-${position}`, initialSize: 180, collapsed });
    }
    const edge = this.edge(position);
    const record = this.panel(panelInstanceId);
    record.moveTo({ group: edge.group.dockviewGroup });
    if (collapsed) edge.group.api.collapse();
  }

  nativeReorder(panelInstanceId: string, index: number): void {
    const record = this.panel(panelInstanceId);
    const panels = record.group.panels;
    const currentIndex = panels.indexOf(record.panel);
    panels.splice(currentIndex, 1);
    panels.splice(index, 0, record.panel);
    this.layoutChange.emit();
  }

  nativeCollapse(position: EdgeGroupPosition, collapsed: boolean): void {
    const edge = this.edge(position);
    if (collapsed) edge.group.api.collapse();
    else edge.group.api.expand();
  }

  layout(): SerializedDockview {
    const gridGroups = [...this.groupRecords.values()].filter(
      (group) => group.location.type === "grid",
    );
    const leaves = gridGroups.map((group) => ({
      type: "leaf" as const,
      data: {
        id: group.id,
        views: group.panels.map((panel) => panel.id),
        activeView: group.activePanel?.id ?? "",
      },
    }));
    const root =
      leaves.length === 1 ? leaves[0] : { type: "branch" as const, data: leaves, size: 100 };
    const edgeGroups = Object.fromEntries(
      [...this.edges.entries()].map(([position, edge]) => [
        position,
        {
          size: position === "left" || position === "right" ? edge.group.width : edge.group.height,
          visible: this.edgeVisibility.get(position) ?? false,
          collapsed: edge.group.collapsed,
          group: {
            id: edge.group.id,
            views: edge.group.panels.map((panel) => panel.id),
            activeView: edge.group.activePanel?.id ?? "",
          },
        },
      ]),
    );

    return {
      grid: {
        root,
        height: 800,
        width: 1200,
        orientation: "HORIZONTAL",
      },
      activeGroup: this.activeGroupRecord?.id,
      panels: Object.fromEntries(
        [...this.panelRecords.entries()].map(([id, record]) => [
          id,
          {
            id,
            contentComponent: record.panel.api.component,
            params: structuredClone(record.params),
            title: record.title,
          },
        ]),
      ),
      edgeGroups,
    } as unknown as SerializedDockview;
  }

  private createGroup(id: string, location: WorkbenchLocation): FakeGroup {
    const collapsedChange = event<{ isCollapsed: boolean }>();
    const fake = this;
    const state = {
      id,
      location,
      panels: [] as IDockviewPanel[],
      recentPanelIds: [] as string[],
      activePanel: undefined as IDockviewPanel | undefined,
      headerPosition: "top" as FakeGroup["headerPosition"],
      width: 300,
      height: 200,
      collapsed: false,
    };
    const expand = vi.fn(() => {
      if (!state.collapsed) return;
      state.collapsed = false;
      collapsedChange.emit({ isCollapsed: false });
    });
    const collapse = vi.fn(() => {
      if (state.collapsed) return;
      state.collapsed = true;
      collapsedChange.emit({ isCollapsed: true });
    });
    let group!: FakeGroup;
    const groupApi = {
      id,
      get width() {
        return state.width;
      },
      get height() {
        return state.height;
      },
      get isActive() {
        return fake.activeGroupRecord === group;
      },
      get location() {
        return state.location;
      },
      onDidCollapsedChange: collapsedChange.subscribe,
      setActive: vi.fn(() => this.setActiveGroup(group)),
      setSize: vi.fn(({ width, height }: { width?: number; height?: number }) => {
        if (width !== undefined) state.width = width;
        if (height !== undefined) state.height = height;
        this.layoutChange.emit();
      }),
      setHeaderPosition: vi.fn((position: "top" | "bottom" | "left" | "right") => {
        state.headerPosition = position;
        this.layoutChange.emit();
      }),
      getHeaderPosition: () => state.headerPosition,
      collapse,
      expand,
      isCollapsed: () => state.collapsed,
    } as unknown as DockviewGroupPanelApi;
    const dockviewGroup = {
      id,
      get panels() {
        return state.panels;
      },
      get activePanel() {
        return state.activePanel;
      },
      get api() {
        return groupApi;
      },
      model: {
        get isActive() {
          return fake.activeGroupRecord === group;
        },
      },
    } as unknown as DockviewGroupPanel;
    group = {
      get activePanel() {
        return state.activePanel;
      },
      set activePanel(panel) {
        state.activePanel = panel;
      },
      get collapsed() {
        return state.collapsed;
      },
      set collapsed(value) {
        state.collapsed = value;
      },
      get headerPosition() {
        return state.headerPosition;
      },
      set headerPosition(value) {
        state.headerPosition = value;
      },
      get height() {
        return state.height;
      },
      set height(value) {
        state.height = value;
      },
      get width() {
        return state.width;
      },
      set width(value) {
        state.width = value;
      },
      id,
      location,
      panels: state.panels,
      recentPanelIds: state.recentPanelIds,
      api: groupApi,
      dockviewGroup,
      collapsedChange,
    };
    this.groupRecords.set(id, group);
    this.groups.push(dockviewGroup);
    return group;
  }

  private createEdge(
    position: EdgeGroupPosition,
    options: { id: string; initialSize?: number; collapsed?: boolean },
  ): DockviewGroupPanelApi {
    if (this.edges.has(position)) throw new Error("duplicate edge");
    const group = this.createGroup(options.id, { type: "edge", position });
    if (position === "left" || position === "right") group.width = options.initialSize ?? 200;
    else group.height = options.initialSize ?? 200;
    group.collapsed = options.collapsed ?? false;
    this.edgeVisibility.set(position, true);
    const edge = {
      group,
      addPanel: vi.fn((_options: AddPanelOptions<Record<string, unknown>>) => undefined),
      expand: group.api.expand as Mock<() => void>,
    };
    this.edges.set(position, edge);
    this.layoutChange.emit();
    return group.api;
  }

  private addPanel(options: AddPanelOptions<Record<string, unknown>>): IDockviewPanel {
    if (this.panelRecords.has(options.id)) throw new Error("duplicate panel");
    const position = options.position as
      | {
          referenceGroup?: string | { id: string };
          index?: number;
        }
      | undefined;
    const referenceId =
      typeof position?.referenceGroup === "string"
        ? position.referenceGroup
        : position?.referenceGroup?.id;
    const group =
      (referenceId ? this.groupRecords.get(referenceId) : undefined) ??
      this.activeGroupRecord ??
      [...this.groupRecords.values()].find((candidate) => candidate.location.type === "grid");
    if (!group) throw new Error("missing target group");

    const record = {} as FakePanelRecord;
    const panelApiState = {
      pinned: false,
    };
    const close = vi.fn(() => this.removePanel(record));
    const panelApi = {
      id: options.id,
      component: options.component,
      get isActive() {
        return record.active;
      },
      get isVisible() {
        return record.group.activePanel === record.panel;
      },
      get isPinned() {
        return panelApiState.pinned;
      },
      get location() {
        return record.group.location;
      },
      setActive: vi.fn(() => this.setActive(record)),
      close,
      updateParameters: vi.fn((params: Record<string, unknown>) => {
        record.params = structuredClone(params);
        this.layoutChange.emit();
      }),
      setTitle: vi.fn((title: string) => {
        record.title = title;
        this.layoutChange.emit();
      }),
      setPinned: vi.fn((pinned: boolean) => {
        panelApiState.pinned = pinned;
        record.pinned = pinned;
        this.layoutChange.emit();
      }),
    };
    const panel = {
      id: options.id,
      get group() {
        return record.group.dockviewGroup;
      },
      get params() {
        return record.params;
      },
      get title() {
        return record.title;
      },
      get isPinned() {
        return record.pinned;
      },
      api: panelApi,
    } as unknown as IDockviewPanel;
    const moveTo = vi.fn((request: FakeMoveRequest) => this.movePanel(record, request));
    Object.assign(record, {
      panel,
      moveTo,
      close,
      setPinned: panelApi.setPinned,
      params: structuredClone(options.params ?? {}),
      title: options.title,
      group,
      active: false,
      pinned: false,
    });
    (panelApi as { moveTo?: typeof moveTo }).moveTo = moveTo;

    this.panels.push(panel);
    const targetIndex = Math.min(position?.index ?? group.panels.length, group.panels.length);
    group.panels.splice(Math.max(0, targetIndex), 0, panel);
    if (group.location.type === "edge") this.edge(group.location.position).addPanel(options);
    this.panelRecords.set(options.id, record);
    this.touchGroupPanel(group, panel.id);
    if (!group.activePanel) group.activePanel = panel;
    if (!options.inactive) this.setActive(record);
    this.layoutChange.emit();
    return panel;
  }

  private setActive(record: FakePanelRecord): void {
    record.group.activePanel = record.panel;
    this.touchGroupPanel(record.group, record.panel.id);
    this.setActiveGroup(record.group);
  }

  private setActiveGroup(group: FakeGroup | undefined): void {
    for (const candidate of this.panelRecords.values()) candidate.active = false;
    this.activeGroupRecord = group;
    const selected = group?.activePanel ? this.panelRecords.get(group.activePanel.id) : undefined;
    this.activePanelRecord = selected;
    if (selected) selected.active = true;
    this.activePanelChange.emit({ panel: selected?.panel });
    this.activeGroupChange.emit(group?.dockviewGroup);
  }

  private touchGroupPanel(group: FakeGroup, panelInstanceId: string): void {
    const previousIndex = group.recentPanelIds.indexOf(panelInstanceId);
    if (previousIndex >= 0) group.recentPanelIds.splice(previousIndex, 1);
    group.recentPanelIds.unshift(panelInstanceId);
  }

  private recentGroupPanel(group: FakeGroup): IDockviewPanel | undefined {
    for (const panelInstanceId of group.recentPanelIds) {
      const record = this.panelRecords.get(panelInstanceId);
      if (record?.group === group && group.panels.includes(record.panel)) return record.panel;
    }
    return group.panels[0];
  }

  private movePanel(record: FakePanelRecord, request: FakeMoveRequest): void {
    const reference = request.group ? this.groupRecords.get(request.group.id) : record.group;
    if (!reference) throw new Error("missing move group");
    let target = reference;
    if (request.position && request.position !== "center") {
      target = this.addGridGroup(`grid-${this.nextGroupId++}`, !request.skipSetActive);
    }

    const wasActive = this.activePanelRecord === record;
    const source = record.group;
    this.detachPanel(record);
    record.group = target;
    const index = Math.min(
      Math.max(request.index ?? target.panels.length, 0),
      target.panels.length,
    );
    target.panels.splice(index, 0, record.panel);
    this.touchGroupPanel(target, record.panel.id);
    if (!target.activePanel) target.activePanel = record.panel;
    if (!request.skipSetActive) this.setActive(record);
    else if (wasActive && this.groupRecords.has(source.id)) this.setActiveGroup(source);
    this.layoutChange.emit();
  }

  private removePanel(record: FakePanelRecord): void {
    const wasActive = this.activePanelRecord === record;
    const source = record.group;
    this.detachPanel(record);
    const panelIndex = this.panels.indexOf(record.panel);
    if (panelIndex >= 0) this.panels.splice(panelIndex, 1);
    this.panelRecords.delete(record.panel.id);
    if (wasActive) {
      const survivingSource = this.groupRecords.get(source.id);
      const fallbackGroup = survivingSource ?? this.groupRecords.values().next().value;
      this.setActiveGroup(fallbackGroup);
    }
    this.layoutChange.emit();
  }

  private detachPanel(record: FakePanelRecord): void {
    const source = record.group;
    const index = source.panels.indexOf(record.panel);
    if (index >= 0) source.panels.splice(index, 1);
    const recentIndex = source.recentPanelIds.indexOf(record.panel.id);
    if (recentIndex >= 0) source.recentPanelIds.splice(recentIndex, 1);
    if (source.activePanel === record.panel) source.activePanel = this.recentGroupPanel(source);
    if (this.activePanelRecord === record) {
      this.activePanelRecord = undefined;
      record.active = false;
    }
    if (source.panels.length === 0 && source.location.type === "grid") {
      this.groupRecords.delete(source.id);
      const groupIndex = this.groups.indexOf(source.dockviewGroup);
      if (groupIndex >= 0) this.groups.splice(groupIndex, 1);
      if (this.activeGroupRecord === source) this.activeGroupRecord = undefined;
    }
  }
}

function createFakeWorkbenchDockview(): FakeWorkbenchDockview {
  return new FakeWorkbenchDockview();
}

const editorRequest = {
  resourceRef: "events/shared.yssbi-event",
  resourceKind: "event",
  title: "Shared event",
  pinned: false,
  mode: "new-instance",
} as const;

describe("workbench Dockview port", () => {
  it("serializes queued singleton and Result operations", async () => {
    const fake = createFakeWorkbenchDockview();
    const { port, internal } = createDockviewHarness();

    const logsA = port.ensureView({ viewId: "logs", title: "Logs" });
    const logsB = port.ensureView({ viewId: "logs", title: "Logs" });
    const resultA = port.upsertResult({
      resultKey: "output:main",
      resultId: "result-1",
      title: "Summary",
      presentation: { kind: "inspector" },
      source: null,
    });
    const resultB = port.upsertResult({
      resultKey: "output:main",
      resultId: "result-2",
      title: "Summary",
      presentation: { kind: "inspector" },
      source: null,
    });

    internal.bind(fake.api);
    internal.completeHydration();
    await Promise.all([logsA, logsB, resultA, resultB]);

    expect(port.listPanels().filter((panel) => panel.metadata.role === "view")).toHaveLength(1);
    const results = port.listPanels().filter((panel) => panel.metadata.role === "result");
    expect(results).toHaveLength(1);
    expect(results[0]?.metadata).toMatchObject({ resultId: "result-2" });
  });

  it("invalidates an unbound project-scoped open before a replacement root hydrates", async () => {
    const replacement = createFakeWorkbenchDockview();
    const { port, internal } = createDockviewHarness();
    const pendingOpen = port.openEditor(editorRequest);
    const idle = internal.whenIdle();

    internal.invalidatePendingOperations();

    await expect(pendingOpen).rejects.toMatchObject({
      code: "dockview_not_ready",
      details: { reason: "stale_binding" },
    } satisfies Partial<WorkbenchLayoutError>);
    await expect(idle).resolves.toBeUndefined();

    const hydrationEpoch = internal.beginHydration();
    internal.bind(replacement.api);
    internal.completeHydration(hydrationEpoch);
    await internal.whenIdle();

    expect(replacement.panels).toHaveLength(0);
    expect(replacement.fromJSON).not.toHaveBeenCalled();
  });

  it("reveals an existing panel in its actual edge without moving it home", async () => {
    const fake = createFakeWorkbenchDockview();
    const { port, internal } = createDockviewHarness();
    internal.bind(fake.api);
    internal.completeHydration();

    const panel = await port.ensureView({ viewId: "logs", title: "Logs" });
    fake.movePanelToEdge(panel.panelInstanceId, "right", true);
    await port.ensureView({ viewId: "logs", title: "Logs" });

    expect(port.getPanel(panel.panelInstanceId)?.groupId).toBe("edge-right");
    expect(fake.edge("right").expand).toHaveBeenCalledOnce();
    expect(fake.edge("bottom").addPanel).toHaveBeenCalledOnce();
  });

  it("publishes native layout, active, and collapse events and rebinds edge listeners", async () => {
    const fake = createFakeWorkbenchDockview();
    const { port, internal } = createDockviewHarness();
    internal.bind(fake.api);
    internal.completeHydration();
    const logs = await port.ensureView({ viewId: "logs", title: "Logs" });
    const output = await port.ensureView({ viewId: "output", title: "Output" });
    let notifications = 0;
    port.subscribe(() => {
      notifications += 1;
    });

    let revision = port.getSnapshot().revision;
    fake.nativeReorder(output.panelInstanceId, 0);
    expect(port.getSnapshot().revision).toBe(revision + 1);
    expect(port.listGroupPanels(logs.groupId).map((panel) => panel.panelInstanceId)).toEqual([
      output.panelInstanceId,
      logs.panelInstanceId,
    ]);

    revision = port.getSnapshot().revision;
    fake.activePanelChange.emit({});
    fake.activeGroupChange.emit(fake.edge("bottom").group.dockviewGroup);
    expect(port.getSnapshot().revision).toBe(revision + 2);

    const collapseSubscriptions =
      fake.edge("bottom").group.collapsedChange.subscribe.mock.calls.length;
    fake.layoutFromJson.emit();
    expect(fake.edge("bottom").group.collapsedChange.subscribe.mock.calls.length).toBeGreaterThan(
      collapseSubscriptions,
    );

    const layoutEvents = fake.layoutChange.emit.mock.calls.length;
    revision = port.getSnapshot().revision;
    fake.nativeCollapse("bottom", true);
    expect(fake.layoutChange.emit).toHaveBeenCalledTimes(layoutEvents);
    expect(port.getSnapshot().revision).toBe(revision + 1);
    expect(port.getEdgeState("bottom").collapsed).toBe(true);
    expect((await port.serialize()).edgeGroups?.bottom?.collapsed).toBe(true);
    expect(notifications).toBeGreaterThanOrEqual(4);
  });

  it("projects panel visibility independently from the globally active panel", async () => {
    const fake = createFakeWorkbenchDockview();
    fake.addGridGroup("grid-secondary", false);
    const { port, internal } = createDockviewHarness();
    internal.bind(fake.api);
    internal.completeHydration();

    const mainPanel = await port.openEditor(editorRequest);
    const splitPanel = await port.openEditor({
      ...editorRequest,
      targetGroupId: "grid-secondary",
    });

    expect(port.getPanel(mainPanel.panelInstanceId)).toMatchObject({
      active: false,
      visible: true,
    });
    expect(port.getPanel(splitPanel.panelInstanceId)).toMatchObject({
      active: true,
      visible: true,
    });
  });

  it("guarantees configured edge identity, geometry, header, and collapse state", async () => {
    const fake = createFakeWorkbenchDockview();
    const { port, internal } = createDockviewHarness();
    internal.bind(fake.api);
    internal.completeHydration();

    const configured = await port.configureEdge({
      position: "bottom",
      size: 220,
      collapsed: true,
    });

    expect(configured).toMatchObject({
      exists: true,
      groupId: "workbench-edge-bottom",
      position: "bottom",
      visible: true,
      collapsed: true,
      size: 220,
    });
    expect(fake.edge("bottom").group.headerPosition).toBe("bottom");
    expect(await port.setEdgeSize("bottom", 260)).toBe(true);
    expect(await port.setEdgeCollapsed("bottom", false)).toBe(true);
    expect(port.getEdgeState("bottom")).toMatchObject({
      exists: true,
      size: 260,
      collapsed: false,
    });
  });

  it("does not call moveTo for a same-group move whose effective index is unchanged", async () => {
    const fake = createFakeWorkbenchDockview();
    const { port, internal } = createDockviewHarness();
    internal.bind(fake.api);
    internal.completeHydration();
    const panel = await port.openEditor(editorRequest);
    const record = fake.panel(panel.panelInstanceId);

    expect(
      await port.move({
        panelInstanceId: panel.panelInstanceId,
        groupId: panel.groupId,
        index: 0,
      }),
    ).toBe(true);

    expect(record.moveTo).not.toHaveBeenCalled();
    expect(port.getPanel(panel.panelInstanceId)?.groupId).toBe("grid-main");
    expect(port.listGroups().some((group) => group.groupId === "grid-main")).toBe(true);
  });

  it("keeps duplicate editor identity, canonical remaps, pinning, and role filters separate", async () => {
    const fake = createFakeWorkbenchDockview();
    const { port, internal } = createDockviewHarness();
    internal.bind(fake.api);
    internal.completeHydration();

    const first = await port.openEditor(editorRequest);
    const second = await port.openEditor(editorRequest);
    fake.panel(first.panelInstanceId).panel.api.updateParameters({
      ...fake.panel(first.panelInstanceId).params,
      layoutTab: { id: editorRequest.resourceRef },
    });
    const result = await port.upsertResult({
      resultKey: "shared-result",
      resultId: "result-1",
      title: "Shared result",
      presentation: { kind: "inspector" },
      source: null,
    });
    await port.move({
      panelInstanceId: result.panelInstanceId,
      groupId: first.groupId,
      activate: true,
    });

    expect(
      port
        .findEditorPanelsByResource(editorRequest.resourceRef)
        .map((panel) => panel.panelInstanceId),
    ).toEqual([first.panelInstanceId, second.panelInstanceId]);
    expect(port.getActiveEditorPanel()).toBeUndefined();
    expect(port.listGroupPanels(first.groupId).map((panel) => panel.metadata.role)).toEqual([
      "editor",
      "editor",
      "result",
    ]);

    expect(await port.remapResource(editorRequest.resourceRef, "events/renamed.yssbi-event")).toBe(
      2,
    );
    expect(port.findEditorPanelsByResource("events/renamed.yssbi-event")).toHaveLength(2);
    expect(fake.panel(first.panelInstanceId).params).toMatchObject({
      layoutTab: { id: editorRequest.resourceRef },
      metadata: { resourceRef: "events/renamed.yssbi-event" },
    });

    expect(await port.setEditorPinned(first.panelInstanceId, true)).toBe(true);
    expect(port.getPanel(first.panelInstanceId)?.metadata).toMatchObject({ pinned: true });
    expect(fake.panel(first.panelInstanceId).pinned).toBe(true);
    expect(fake.panel(first.panelInstanceId).setPinned).toHaveBeenCalledWith(true);

    const reused = await port.openEditor({
      ...editorRequest,
      resourceRef: "events/renamed.yssbi-event",
      mode: "reuse-resource",
    });
    expect(reused.panelInstanceId).toBe(first.panelInstanceId);
    expect(port.findEditorPanelsByResource("events/renamed.yssbi-event")).toHaveLength(2);
  });

  it("rejects reuse when the existing editor renderer has a different resource kind", async () => {
    const fake = createFakeWorkbenchDockview();
    const { port, internal } = createDockviewHarness();
    internal.bind(fake.api);
    internal.completeHydration();
    const existing = await port.openEditor(editorRequest);

    await expect(
      port.openEditor({
        ...editorRequest,
        resourceKind: "chart",
        title: "Shared chart",
        mode: "reuse-resource",
      }),
    ).rejects.toMatchObject({
      code: "panel_open_failed",
      details: { panelInstanceId: existing.panelInstanceId },
    } satisfies Partial<WorkbenchLayoutError>);

    expect(port.listPanels()).toHaveLength(1);
    expect(port.getPanel(existing.panelInstanceId)).toMatchObject({
      component: "EditorResource",
      metadata: { resourceKind: "event" },
    });
    expect(fake.panel(existing.panelInstanceId).panel.api.component).toBe("EditorResource");
  });

  it("revalidates commit tokens inside the FIFO and resolves whenIdle after queued work", async () => {
    const fake = createFakeWorkbenchDockview();
    const { port, internal } = createDockviewHarness();
    internal.bind(fake.api);
    internal.completeHydration();
    const panel = await port.openEditor(editorRequest);
    const token = {
      panelInstanceId: panel.panelInstanceId,
      groupId: panel.groupId,
      metadata: panel.metadata,
    };
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const publication = internal.runPublicationTransaction(async (transaction) => {
      transaction.remapResource(editorRequest.resourceRef, "events/new.yssbi-event");
      await gate;
    });
    const removal = internal.commitRemove([token]);
    let idle = false;
    const idlePromise = internal.whenIdle().then(() => {
      idle = true;
    });

    await Promise.resolve();
    expect(idle).toBe(false);
    expect(port.getPanel(panel.panelInstanceId)?.metadata).toMatchObject({
      resourceRef: editorRequest.resourceRef,
    });

    release();
    await publication;
    expect(await removal).toBe("stale");
    await idlePromise;
    expect(idle).toBe(true);
    expect(port.getPanel(panel.panelInstanceId)?.metadata).toMatchObject({
      resourceRef: "events/new.yssbi-event",
    });
  });

  it("rejects old-bound queued work immediately when its root unbinds", async () => {
    const firstFake = createFakeWorkbenchDockview();
    const replacementFake = createFakeWorkbenchDockview();
    const { port, internal } = createDockviewHarness();
    internal.bind(firstFake.api);
    internal.completeHydration();
    let release!: () => void;
    let markStarted!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const started = new Promise<void>((resolve) => {
      markStarted = resolve;
    });
    const publication = internal.runPublicationTransaction(async () => {
      markStarted();
      await gate;
    });
    const publicationOutcome = publication.then(
      () => undefined,
      (error: unknown) => error,
    );
    await started;

    let queuedSettled = false;
    const queuedOutcome = port
      .openEditor({
        ...editorRequest,
        resourceRef: "events/old-bound.yssbi-event",
      })
      .then(
        (value) => {
          queuedSettled = true;
          return { status: "resolved" as const, value };
        },
        (error: unknown) => {
          queuedSettled = true;
          return { status: "rejected" as const, error };
        },
      );
    const idle = internal.whenIdle();

    internal.unbind(firstFake.api);
    await Promise.resolve();
    const settledOnUnbind = queuedSettled;

    release();
    const publicationError = await publicationOutcome;
    const hydrationEpoch = internal.beginHydration();
    internal.bind(replacementFake.api);
    internal.completeHydration(hydrationEpoch);
    const outcome = await queuedOutcome;
    await idle;

    expect(settledOnUnbind).toBe(true);
    expect(outcome).toMatchObject({
      status: "rejected",
      error: {
        code: "dockview_not_ready",
        details: { reason: "stale_binding" },
      },
    });
    expect(publicationError).toMatchObject({
      code: "dockview_not_ready",
      details: { reason: "stale_binding" },
    });
    expect(firstFake.panels).toHaveLength(0);
    expect(replacementFake.panels).toHaveLength(0);
  });

  it("authorizes committed removal inside the FIFO and treats denial or throw as stale", async () => {
    const fake = createFakeWorkbenchDockview();
    const { port, internal } = createDockviewHarness();
    internal.bind(fake.api);
    internal.completeHydration();
    const first = await port.openEditor(editorRequest);
    const second = await port.openEditor(editorRequest);
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const queued = internal.runPublicationTransaction(async () => {
      await gate;
    });
    let authorized = true;
    const deniedRemoval = internal.commitRemove(
      [
        {
          panelInstanceId: first.panelInstanceId,
          groupId: first.groupId,
          metadata: first.metadata,
        },
      ],
      () => authorized,
    );

    authorized = false;
    release();
    await queued;

    expect(await deniedRemoval).toBe("stale");
    expect(
      await internal.commitRemove(
        [
          {
            panelInstanceId: second.panelInstanceId,
            groupId: second.groupId,
            metadata: second.metadata,
          },
        ],
        () => {
          throw new Error("authorization failed");
        },
      ),
    ).toBe("stale");
    expect(fake.panel(first.panelInstanceId).close).not.toHaveBeenCalled();
    expect(fake.panel(second.panelInstanceId).close).not.toHaveBeenCalled();
    expect(port.getPanel(first.panelInstanceId)).toBeDefined();
    expect(port.getPanel(second.panelInstanceId)).toBeDefined();
  });

  it("prevalidates every commitRemove token before closing the first panel", async () => {
    const fake = createFakeWorkbenchDockview();
    const { port, internal } = createDockviewHarness();
    internal.bind(fake.api);
    internal.completeHydration();
    const first = await port.openEditor(editorRequest);
    const second = await port.openEditor(editorRequest);

    expect(
      await internal.commitRemove([
        {
          panelInstanceId: first.panelInstanceId,
          groupId: first.groupId,
          metadata: first.metadata,
        },
        {
          panelInstanceId: second.panelInstanceId,
          groupId: "stale-group",
          metadata: second.metadata,
        },
      ]),
    ).toBe("stale");

    expect(fake.panel(first.panelInstanceId).close).not.toHaveBeenCalled();
    expect(port.getPanel(first.panelInstanceId)).toBeDefined();
    expect(port.getPanel(second.panelInstanceId)).toBeDefined();
  });

  it("maps Dockview close failures to a stable error with the panel ID", async () => {
    const fake = createFakeWorkbenchDockview();
    const { port, internal } = createDockviewHarness();
    internal.bind(fake.api);
    internal.completeHydration();
    const panel = await port.openEditor(editorRequest);
    fake.panel(panel.panelInstanceId).close.mockImplementationOnce(() => {
      throw new Error("raw Dockview close failure");
    });

    const error = await internal
      .commitRemove([
        {
          panelInstanceId: panel.panelInstanceId,
          groupId: panel.groupId,
          metadata: panel.metadata,
        },
      ])
      .then(
        () => undefined,
        (reason: unknown) => reason,
      );

    expect(error).toBeInstanceOf(WorkbenchLayoutError);
    expect(error).toMatchObject({
      code: "layout_restore_failed",
      details: { panelInstanceId: panel.panelInstanceId },
    });
    expect((error as Error).message).toBe("layout_restore_failed");
    expect(port.getPanel(panel.panelInstanceId)).toBeDefined();
  });

  it("rejects a suspended publication after the root binding changes", async () => {
    const firstFake = createFakeWorkbenchDockview();
    const replacementFake = createFakeWorkbenchDockview();
    const { port, internal } = createDockviewHarness();
    internal.bind(firstFake.api);
    internal.completeHydration();
    await port.openEditor(editorRequest);
    const before = JSON.stringify(firstFake.layout());
    let release!: () => void;
    let markStarted!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const started = new Promise<void>((resolve) => {
      markStarted = resolve;
    });
    const publication = internal.runPublicationTransaction(async (transaction) => {
      transaction.remapResource(editorRequest.resourceRef, "events/stale-binding.yssbi-event");
      markStarted();
      await gate;
    });

    await started;
    internal.unbind(firstFake.api);
    internal.bind(replacementFake.api);
    release();

    await expect(publication).rejects.toMatchObject({
      code: "dockview_not_ready",
      details: { reason: "stale_binding" },
    } satisfies Partial<WorkbenchLayoutError>);
    expect(JSON.stringify(firstFake.layout())).toBe(before);
    expect(replacementFake.panels).toHaveLength(0);
  });

  it("rejects a suspended publication after hydration is invalidated", async () => {
    const fake = createFakeWorkbenchDockview();
    const { port, internal } = createDockviewHarness();
    internal.bind(fake.api);
    internal.completeHydration();
    await port.openEditor(editorRequest);
    const before = JSON.stringify(fake.layout());
    let release!: () => void;
    let markStarted!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const started = new Promise<void>((resolve) => {
      markStarted = resolve;
    });
    const publication = internal.runPublicationTransaction(async (transaction) => {
      transaction.remapResource(editorRequest.resourceRef, "events/stale-hydration.yssbi-event");
      markStarted();
      await gate;
    });

    await started;
    internal.invalidateHydration();
    release();

    await expect(publication).rejects.toMatchObject({
      code: "dockview_not_ready",
      details: { reason: "stale_hydration" },
    } satisfies Partial<WorkbenchLayoutError>);
    expect(JSON.stringify(fake.layout())).toBe(before);
    internal.completeHydration();
  });

  it("rejects a suspended publication invalidated before live apply", async () => {
    const fake = createFakeWorkbenchDockview();
    const { port, internal } = createDockviewHarness();
    internal.bind(fake.api);
    internal.completeHydration();
    await port.openEditor(editorRequest);
    const before = JSON.stringify(fake.layout());
    let release!: () => void;
    let markStarted!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const started = new Promise<void>((resolve) => {
      markStarted = resolve;
    });
    const publication = internal.runPublicationTransaction(async (transaction) => {
      transaction.remapResource(editorRequest.resourceRef, "events/stale-operation.yssbi-event");
      markStarted();
      await gate;
    });
    const idle = internal.whenIdle();

    await started;
    internal.invalidatePendingOperations();
    release();

    await expect(publication).rejects.toMatchObject({
      code: "dockview_not_ready",
      details: { reason: "stale_binding" },
    } satisfies Partial<WorkbenchLayoutError>);
    await idle;
    expect(JSON.stringify(fake.layout())).toBe(before);
  });

  it("installs hydration defaults before draining queued application work", async () => {
    const fake = createFakeWorkbenchDockview();
    const { port, internal } = createDockviewHarness();
    const hydrationEpoch = internal.beginHydration();
    internal.bind(fake.api);
    const observedHydrationStates: boolean[] = [];
    port.subscribe(() => {
      observedHydrationStates.push(port.isHydrated);
    });
    let externalSettled = false;
    const externalOpen = port
      .openEditor({
        ...editorRequest,
        resourceRef: "events/queued-during-hydration.yssbi-event",
      })
      .then((panel) => {
        externalSettled = true;
        return panel;
      });

    const defaults = internal.installHydrationLayout(hydrationEpoch, (transaction) => {
      expect(port.isHydrated).toBe(false);
      transaction.ensureCentralGroup();
      const project = transaction.ensureView({ viewId: "project", title: "Project" });
      const nodes = transaction.ensureView({ viewId: "nodes", title: "Nodes" });
      const data = transaction.ensureView({ viewId: "data", title: "Data" });
      const commands = transaction.ensureView({ viewId: "commands", title: "Commands" });
      const logs = transaction.ensureView({ viewId: "logs", title: "Logs" });
      const output = transaction.ensureView({ viewId: "output", title: "Output" });
      const left = transaction.configureEdge({
        position: "left",
        size: 292,
        collapsed: false,
        headerPosition: "left",
      });
      const bottom = transaction.configureEdge({
        position: "bottom",
        size: 200,
        collapsed: false,
        headerPosition: "bottom",
      });
      [project, nodes, data, commands].forEach((panel, index) => {
        transaction.move({
          panelInstanceId: panel.panelInstanceId,
          groupId: left.groupId,
          index,
        });
      });
      transaction.move({
        panelInstanceId: logs.panelInstanceId,
        groupId: bottom.groupId,
        index: 0,
      });
      transaction.move({
        panelInstanceId: output.panelInstanceId,
        groupId: bottom.groupId,
        index: 1,
      });
      expect(port.isHydrated).toBe(false);
      return { project, nodes, data, commands, logs, output };
    });

    expect(externalSettled).toBe(false);
    expect(port.isHydrated).toBe(false);
    expect(port.listPanels().map((panel) => panel.panelInstanceId)).toEqual([
      defaults.project.panelInstanceId,
      defaults.nodes.panelInstanceId,
      defaults.data.panelInstanceId,
      defaults.commands.panelInstanceId,
      defaults.logs.panelInstanceId,
      defaults.output.panelInstanceId,
    ]);
    expect(port.getPanel(defaults.project.panelInstanceId)?.location).toEqual({
      type: "edge",
      position: "left",
    });
    expect(port.getPanel(defaults.logs.panelInstanceId)?.location).toEqual({
      type: "edge",
      position: "bottom",
    });
    expect(port.getPanel(defaults.output.panelInstanceId)?.location).toEqual({
      type: "edge",
      position: "bottom",
    });
    expect(fake.edge("bottom").group.headerPosition).toBe("bottom");
    expect(observedHydrationStates.length).toBeGreaterThan(0);
    expect(observedHydrationStates.every((state) => state === false)).toBe(true);
    expect(fake.fromJSON).not.toHaveBeenCalled();

    internal.completeHydration(hydrationEpoch);
    const external = await externalOpen;

    expect(externalSettled).toBe(true);
    expect(port.isHydrated).toBe(true);
    expect(port.getPanel(external.panelInstanceId)?.metadata).toMatchObject({
      role: "editor",
      resourceRef: "events/queued-during-hydration.yssbi-event",
    });
  });

  it("commits a valid layout transaction only after its synchronous callback", async () => {
    const fake = createFakeWorkbenchDockview();
    const { port, internal } = createDockviewHarness();
    internal.bind(fake.api);
    internal.completeHydration();

    const panelInstanceId = await internal.runLayoutTransaction((transaction) => {
      const view = transaction.ensureView({ viewId: "logs", title: "Logs" });
      transaction.configureEdge({ position: "bottom", size: 240, collapsed: true });
      const shadowLayout = transaction.serialize();

      expect(fake.panels).toHaveLength(0);
      expect(transaction.getPanel(view.panelInstanceId)?.metadata).toEqual({
        role: "view",
        viewId: "logs",
      });
      expect(shadowLayout.edgeGroups?.bottom).toMatchObject({
        size: 240,
        collapsed: true,
      });
      return view.panelInstanceId;
    });

    expect(port.getPanel(panelInstanceId)?.metadata).toEqual({
      role: "view",
      viewId: "logs",
    });
    expect(port.getEdgeState("bottom")).toMatchObject({
      size: 240,
      collapsed: true,
    });
    expect(fake.fromJSON).not.toHaveBeenCalled();
  });

  it("keeps shadow and serialized active selection coherent through reveal, activate, and remove", async () => {
    const fake = createFakeWorkbenchDockview();
    const { port, internal } = createDockviewHarness();
    internal.bind(fake.api);
    internal.completeHydration();
    const first = await port.openEditor(editorRequest);
    const second = await port.openEditor(editorRequest);

    await internal.runLayoutTransaction((transaction) => {
      const logs = transaction.ensureView({ viewId: "logs", title: "Logs" });
      expect(transaction.getActivePanel()?.panelInstanceId).toBe(logs.panelInstanceId);
      const revealedLayout = transaction.serialize();
      expect(revealedLayout.activeGroup).toBe(logs.groupId);
      expect(revealedLayout.edgeGroups?.bottom?.group).toMatchObject({
        activeView: logs.panelInstanceId,
      });

      expect(transaction.activate(first.panelInstanceId)).toBe(true);
      transaction.removePanels([first.panelInstanceId]);
      expect(transaction.getActivePanel()?.panelInstanceId).toBe(second.panelInstanceId);
      expect(
        transaction.listGroups().find((group) => group.groupId === second.groupId),
      ).toMatchObject({
        active: true,
        activePanelInstanceId: second.panelInstanceId,
      });
      const finalLayout = transaction.serialize();
      expect(finalLayout.activeGroup).toBe(second.groupId);
      expect(finalLayout.grid.root).toMatchObject({
        type: "leaf",
        data: { activeView: second.panelInstanceId },
      });
    });

    expect(port.getActivePanel()?.panelInstanceId).toBe(second.panelInstanceId);
    expect((await port.serialize()).activeGroup).toBe(second.groupId);
  });

  it("reconciles an active empty central group after removing the active sole panel", async () => {
    const fake = createFakeWorkbenchDockview();
    const { port, internal } = createDockviewHarness();
    internal.bind(fake.api);
    internal.completeHydration();
    const panel = await port.openEditor(editorRequest);

    const shadow = await internal.runLayoutTransaction((transaction) => {
      transaction.removePanels([panel.panelInstanceId]);
      const centralGroupId = transaction.ensureCentralGroup();
      const centralGroup = transaction
        .listGroups()
        .find((group) => group.groupId === centralGroupId);

      expect(centralGroupId).not.toBe(panel.groupId);
      expect(centralGroup).toEqual({
        groupId: centralGroupId,
        panelInstanceIds: [],
        active: true,
        location: { type: "grid" },
      });
      expect(transaction.getActivePanel()).toBeUndefined();
      expect(transaction.serialize().activeGroup).toBe(centralGroupId);

      return {
        activePanel: transaction.getActivePanel(),
        groups: transaction.listGroups(),
        layout: transaction.serialize(),
      };
    });

    expect(port.getActivePanel()).toEqual(shadow.activePanel);
    expect(port.listGroups()).toEqual(shadow.groups);
    expect(await port.serialize()).toEqual(shadow.layout);
  });

  it("replays non-last removals before dependent moves and defers last-panel closes", async () => {
    const fake = createFakeWorkbenchDockview();
    const { port, internal } = createDockviewHarness();
    internal.bind(fake.api);
    internal.completeHydration();
    const first = await port.openEditor(editorRequest);
    const second = await port.openEditor(editorRequest);
    const third = await port.openEditor(editorRequest);
    fake.addGridGroup("grid-secondary");
    const soleSecondary = await port.openEditor({
      ...editorRequest,
      targetGroupId: "grid-secondary",
    });

    await internal.runLayoutTransaction((transaction) => {
      transaction.removePanels([first.panelInstanceId, soleSecondary.panelInstanceId]);
      expect(
        transaction.move({
          panelInstanceId: second.panelInstanceId,
          groupId: first.groupId,
          index: 1,
          activate: false,
        }),
      ).toBe(true);
      expect(
        transaction.listGroupPanels(first.groupId).map((panel) => panel.panelInstanceId),
      ).toEqual([third.panelInstanceId, second.panelInstanceId]);
    });

    expect(port.listGroupPanels(first.groupId).map((panel) => panel.panelInstanceId)).toEqual([
      third.panelInstanceId,
      second.panelInstanceId,
    ]);
    expect(port.listGroups().some((group) => group.groupId === "grid-secondary")).toBe(false);
  });

  it("defers subscriber callbacks until buffered live commands reach the final state", async () => {
    const fake = createFakeWorkbenchDockview();
    const { port, internal } = createDockviewHarness();
    internal.bind(fake.api);
    internal.completeHydration();
    const first = await port.openEditor(editorRequest);
    const second = await port.openEditor(editorRequest);
    const third = await port.openEditor(editorRequest);
    const observedOrders: string[][] = [];
    const revision = port.getSnapshot().revision;
    const unsubscribe = port.subscribe(() => {
      observedOrders.push(
        port.listGroupPanels(first.groupId).map((panel) => panel.panelInstanceId),
      );
    });

    await internal.runLayoutTransaction((transaction) => {
      transaction.removePanels([first.panelInstanceId]);
      transaction.move({
        panelInstanceId: second.panelInstanceId,
        groupId: first.groupId,
        index: 1,
        activate: false,
      });
    });
    unsubscribe();

    expect(observedOrders).toEqual([[third.panelInstanceId, second.panelInstanceId]]);
    expect(port.getSnapshot().revision - revision).toBeGreaterThanOrEqual(2);
  });

  it("rejects a missing publication removal before later business work runs", async () => {
    const fake = createFakeWorkbenchDockview();
    const { internal } = createDockviewHarness();
    internal.bind(fake.api);
    internal.completeHydration();
    let businessCommitted = false;

    await expect(
      internal.runPublicationTransaction(async (transaction) => {
        transaction.removePanels(["missing-panel"]);
        businessCommitted = true;
        await Promise.resolve();
      }),
    ).rejects.toMatchObject({
      code: "layout_restore_failed",
      details: {
        reason: "invalid_remove_target",
        panelInstanceId: "missing-panel",
      },
    } satisfies Partial<WorkbenchLayoutError>);

    expect(businessCommitted).toBe(false);
  });

  it("rejects a noncanonical publication removal before later business work runs", async () => {
    const fake = createFakeWorkbenchDockview();
    const { internal } = createDockviewHarness();
    internal.bind(fake.api);
    internal.completeHydration();
    fake.api.addPanel({
      id: "foreign-panel",
      component: "EditorResource",
      params: {},
      position: { referenceGroup: "grid-main" },
    });
    let businessCommitted = false;

    await expect(
      internal.runPublicationTransaction(async (transaction) => {
        transaction.removePanels(["foreign-panel"]);
        businessCommitted = true;
        await Promise.resolve();
      }),
    ).rejects.toMatchObject({
      code: "layout_restore_failed",
      details: {
        reason: "invalid_remove_target",
        panelInstanceId: "foreign-panel",
      },
    } satisfies Partial<WorkbenchLayoutError>);

    expect(businessCommitted).toBe(false);
    expect(fake.api.getPanel("foreign-panel")).toBeDefined();
  });

  it("maps Dockview snapshot and validation serialization failures to stable errors", async () => {
    const snapshotFake = createFakeWorkbenchDockview();
    const snapshotPort = createDockviewHarness();
    snapshotPort.internal.bind(snapshotFake.api);
    snapshotPort.internal.completeHydration();
    snapshotFake.toJSON.mockImplementationOnce(() => {
      throw new Error("raw Dockview snapshot failure");
    });

    const snapshotError = await snapshotPort.internal
      .runLayoutTransaction(() => undefined)
      .then(
        () => undefined,
        (reason: unknown) => reason,
      );
    expect(snapshotError).toBeInstanceOf(WorkbenchLayoutError);
    expect(snapshotError).toMatchObject({ code: "layout_restore_failed", details: {} });
    expect((snapshotError as Error).message).toBe("layout_restore_failed");

    const validationFake = createFakeWorkbenchDockview();
    const validationPort = createDockviewHarness();
    validationPort.internal.bind(validationFake.api);
    validationPort.internal.completeHydration();
    validationFake.toJSON
      .mockImplementationOnce(() => validationFake.layout())
      .mockImplementationOnce(() => {
        throw new Error("raw Dockview validation failure");
      });

    const validationError = await validationPort.internal
      .runLayoutTransaction(() => undefined)
      .then(
        () => undefined,
        (reason: unknown) => reason,
      );
    expect(validationError).toBeInstanceOf(WorkbenchLayoutError);
    expect(validationError).toMatchObject({ code: "layout_restore_failed", details: {} });
    expect((validationError as Error).message).toBe("layout_restore_failed");
  });

  it("applies no live commands when a transaction callback or shadow validation fails", async () => {
    const fake = createFakeWorkbenchDockview();
    const { port, internal } = createDockviewHarness();
    internal.bind(fake.api);
    internal.completeHydration();
    const first = await port.openEditor(editorRequest);
    const second = await port.openEditor(editorRequest);
    const before = JSON.stringify(fake.layout());

    await expect(
      internal.runPublicationTransaction(async (transaction) => {
        transaction.remapResource(editorRequest.resourceRef, "events/rejected.yssbi-event");
        transaction.removePanels([second.panelInstanceId]);
        await Promise.resolve();
        throw new Error("business commit failed");
      }),
    ).rejects.toThrow("business commit failed");
    expect(JSON.stringify(fake.layout())).toBe(before);

    await expect(
      internal.runLayoutTransaction((transaction) => {
        transaction.move({
          panelInstanceId: first.panelInstanceId,
          groupId: first.groupId,
          index: -1,
        });
      }),
    ).rejects.toMatchObject({
      code: "layout_restore_failed",
    } satisfies Partial<WorkbenchLayoutError>);
    expect(JSON.stringify(fake.layout())).toBe(before);
    expect(fake.fromJSON).not.toHaveBeenCalled();
  });

  it("keeps lifecycle, hydration install, and restore capabilities off public interfaces", () => {
    const { port, internal } = createDockviewHarness();

    expect(port).not.toHaveProperty("remove");
    expect(port).not.toHaveProperty("restore");
    expect(port).not.toHaveProperty("installHydrationLayout");
    expect(port).not.toHaveProperty("invalidatePendingOperations");
    expect(internal).toHaveProperty("installHydrationLayout");
    expect(internal).toHaveProperty("invalidatePendingOperations");
    expect(internal).not.toHaveProperty("restore");
    expect(publicDockview).not.toHaveProperty("createWorkbenchDockviewRuntime");
    expect(publicDockview).not.toHaveProperty("workbenchDockviewInternal");
    expect(publicDockview).not.toHaveProperty("installHydrationLayout");
    expect(publicDockview).not.toHaveProperty("invalidatePendingOperations");
  });
});
