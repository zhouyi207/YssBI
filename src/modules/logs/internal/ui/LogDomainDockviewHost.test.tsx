// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import type { DockviewApi } from "dockview-react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { DiagnosticRecordDto } from "@/shared/types/domain/diagnostics";

const mocks = vi.hoisted(() => ({
  dockviews: [] as unknown[],
  entries: [] as DiagnosticRecordDto[],
  subscribeDiagnostics: vi.fn(),
  activateSubscription: vi.fn(),
  unsubscribeDiagnostics: vi.fn(async () => {}),
}));

vi.mock("dockview-react", async (importOriginal) => {
  const actual = await importOriginal<typeof import("dockview-react")>();
  const React = await import("react");

  type FakeGroup = {
    id: string;
    api: { id: string };
    panels: FakePanel[];
    activePanel: FakePanel | undefined;
  };
  type FakePanel = {
    id: string;
    component: string;
    title: string | undefined;
    params: Record<string, unknown> | undefined;
    group: FakeGroup;
    api: {
      id: string;
      readonly group: FakeGroup;
      readonly title: string | undefined;
      setActive: () => void;
      setTitle: (title: string) => void;
      onDidTitleChange: (listener: (event: { title: string | undefined }) => void) => {
        dispose: () => void;
      };
      moveTo: (options: { group?: FakeGroup }) => void;
    };
  };

  function FakeDockviewReact(props: Record<string, any>) {
    const [, forceRender] = React.useReducer((value) => value + 1, 0);
    const instanceRef = React.useRef<any>(null);

    if (!instanceRef.current) {
      const layoutListeners = new Set<() => void>();
      const panelsById = new Map<string, FakePanel>();
      let groups: FakeGroup[] = [];
      let activeGroupId: string | undefined;
      let nextGroupId = 1;

      const publishLayout = () => {
        forceRender();
        [...layoutListeners].forEach((listener) => listener());
      };

      const activatePanel = (panel: FakePanel, publish = true) => {
        panel.group.activePanel = panel;
        activeGroupId = panel.group.id;
        if (publish) publishLayout();
      };

      const addGroupInternal = (id?: string, publish = true): FakeGroup => {
        const groupId = id ?? `logs-test-group-${nextGroupId++}`;
        if (groups.some((group) => group.id === groupId)) {
          throw new Error(`duplicate Dockview group id: ${groupId}`);
        }
        const group: FakeGroup = {
          id: groupId,
          api: { id: groupId },
          panels: [],
          activePanel: undefined,
        };
        groups = [...groups, group];
        activeGroupId = group.id;
        if (publish) publishLayout();
        return group;
      };

      const movePanelInternal = (panel: FakePanel, target: FakeGroup, publish = true) => {
        const source = panel.group;
        source.panels = source.panels.filter((candidate) => candidate !== panel);
        if (source.activePanel === panel) source.activePanel = source.panels[0];
        if (source.panels.length === 0) groups = groups.filter((group) => group !== source);
        panel.group = target;
        target.panels = [...target.panels, panel];
        activatePanel(panel, false);
        if (publish) publishLayout();
      };

      const addPanelInternal = (
        options: Record<string, any>,
        target: FakeGroup,
        publish = true,
      ): FakePanel => {
        if (panelsById.has(options.id)) {
          throw new Error(`duplicate Dockview panel id: ${options.id}`);
        }
        const domain = options.params?.domain;
        if (options.component === "LogDomainPanel" && options.id !== `logs-domain:${domain}`) {
          throw new Error(`noncanonical log domain panel id: ${options.id}`);
        }

        const panel = {} as FakePanel;
        const titleListeners = new Set<(event: { title: string | undefined }) => void>();
        panel.id = options.id;
        panel.component = options.component;
        panel.title = options.title;
        panel.params = options.params;
        panel.group = target;
        panel.api = {
          id: panel.id,
          get group() {
            return panel.group;
          },
          get title() {
            return panel.title;
          },
          setActive: () => activatePanel(panel),
          setTitle: (title) => {
            if (panel.title === title) return;
            panel.title = title;
            [...titleListeners].forEach((listener) => listener({ title }));
          },
          onDidTitleChange: (listener) => {
            titleListeners.add(listener);
            return { dispose: () => titleListeners.delete(listener) };
          },
          moveTo: ({ group }) => {
            if (!group) throw new Error("fake Dockview moveTo requires a target group");
            movePanelInternal(panel, group);
          },
        };
        panelsById.set(panel.id, panel);
        target.panels = [...target.panels, panel];
        if (!options.inactive) activatePanel(panel, false);
        if (publish) publishLayout();
        return panel;
      };

      const api = {
        get panels() {
          return [...panelsById.values()];
        },
        get groups() {
          return groups;
        },
        get activeGroup() {
          return groups.find((group) => group.id === activeGroupId);
        },
        get activePanel() {
          return this.activeGroup?.activePanel;
        },
        getPanel: (id: string) => panelsById.get(id),
        getGroup: (id: string) => groups.find((group) => group.id === id),
        onDidLayoutChange: (listener: () => void) => {
          layoutListeners.add(listener);
          return { dispose: () => layoutListeners.delete(listener) };
        },
        addGroup: (options: { id?: string } = {}) => addGroupInternal(options.id),
        fromJSON: (layout: Record<string, any>) => {
          instanceRef.current.fromJSONInputs.push(layout);
          if (layout.grid?.root?.type !== "branch" || !Array.isArray(layout.grid.root.data)) {
            throw new Error("fake Dockview requires a branch-root restore");
          }

          const leaves: Array<Record<string, any>> = [];
          const visit = (node: Record<string, any>) => {
            if (node.type === "leaf") {
              leaves.push(node.data);
              return;
            }
            if (node.type !== "branch" || !Array.isArray(node.data)) {
              throw new Error("invalid fake Dockview grid node");
            }
            node.data.forEach(visit);
          };
          layout.grid.root.data.forEach(visit);

          panelsById.clear();
          groups = [];
          activeGroupId = undefined;
          const referencedPanels = new Set<string>();
          for (const leaf of leaves) {
            const group = addGroupInternal(leaf.id, false);
            for (const panelId of leaf.views ?? []) {
              if (referencedPanels.has(panelId))
                throw new Error(`duplicate panel view: ${panelId}`);
              const state = layout.panels?.[panelId];
              if (!state || state.id !== panelId)
                throw new Error(`missing panel state: ${panelId}`);
              referencedPanels.add(panelId);
              addPanelInternal(
                {
                  id: panelId,
                  component: state.contentComponent,
                  title: state.title,
                  params: state.params,
                  inactive: true,
                },
                group,
                false,
              );
            }
            group.activePanel =
              group.panels.find((panel) => panel.id === leaf.activeView) ?? group.panels[0];
          }
          if (referencedPanels.size !== Object.keys(layout.panels ?? {}).length) {
            throw new Error("fake Dockview restore contains unreferenced panels");
          }
          activeGroupId = layout.activeGroup ?? groups[0]?.id;
          publishLayout();
        },
        toJSON: () => ({
          grid: {
            root: {
              type: "branch",
              data: groups.map((group) => ({
                type: "leaf",
                data: {
                  id: group.id,
                  views: group.panels.map((panel) => panel.id),
                  ...(group.activePanel ? { activeView: group.activePanel.id } : {}),
                },
              })),
            },
            height: 600,
            width: 1000,
            orientation: actual.Orientation.HORIZONTAL,
          },
          panels: Object.fromEntries(
            [...panelsById.values()].map((panel) => [
              panel.id,
              {
                id: panel.id,
                contentComponent: panel.component,
                title: panel.title,
                params: panel.params,
              },
            ]),
          ),
          ...(activeGroupId ? { activeGroup: activeGroupId } : {}),
          floatingGroups: [],
          popoutGroups: [],
        }),
      };

      instanceRef.current = {
        api,
        props,
        fromJSONInputs: [] as Array<Record<string, any>>,
      };
    }

    const instance = instanceRef.current;
    instance.props = props;

    React.useLayoutEffect(() => {
      mocks.dockviews.push(instance);
      props.onReady({ api: instance.api });
    }, []);

    const Watermark = props.watermarkComponent;
    const Actions = props.rightHeaderActionsComponent;
    const DefaultTab = props.defaultTabComponent as
      | React.ComponentType<Record<string, unknown>>
      | undefined;
    const renderedGroups = instance.api.groups.map((group: FakeGroup) => {
      const activePanel = group.activePanel;
      const Panel = activePanel ? props.components[activePanel.component] : undefined;
      return React.createElement(
        "section",
        { key: group.id, "data-group-id": group.id },
        React.createElement(
          "div",
          { role: "tablist" },
          group.panels.map((panel) =>
            React.createElement(
              "button",
              {
                key: panel.id,
                role: "tab",
                "aria-selected": panel === activePanel,
                onClick: () => panel.api.setActive(),
              },
              DefaultTab
                ? React.createElement(DefaultTab, {
                    api: panel.api,
                    containerApi: instance.api,
                    params: panel.params,
                    tabLocation: "header",
                  })
                : panel.title,
            ),
          ),
        ),
        Actions
          ? React.createElement(Actions, {
              api: group.api,
              containerApi: instance.api,
              panels: group.panels,
              activePanel,
              isGroupActive: instance.api.activeGroup === group,
              group,
              headerPosition: "top",
            })
          : null,
        activePanel && Panel
          ? React.createElement(
              "div",
              { "data-panel-id": activePanel.id },
              React.createElement(Panel, {
                api: activePanel.api,
                containerApi: instance.api,
                params: activePanel.params,
              }),
            )
          : Watermark
            ? React.createElement(Watermark, { containerApi: instance.api, group })
            : null,
      );
    });

    return React.createElement(
      "div",
      {
        className: props.className,
        "data-fake-dockview": true,
      },
      renderedGroups.length > 0
        ? renderedGroups
        : Watermark
          ? React.createElement(Watermark, { containerApi: instance.api })
          : null,
    );
  }

  return { ...actual, DockviewReact: FakeDockviewReact };
});

vi.mock("@/services/log", () => ({
  LogService: {
    subscribeDiagnostics: mocks.subscribeDiagnostics,
  },
}));

vi.mock("react-i18next", async (importOriginal) => {
  const actual = await importOriginal<typeof import("react-i18next")>();
  return {
    ...actual,
    useTranslation: () => ({ t: (key: string) => key }),
  };
});

vi.mock("./LogPanelList", async () => {
  const React = await import("react");
  return {
    LogPanelList: (props: Record<string, any>) =>
      React.createElement("div", {
        "data-log-panel-list": true,
        "data-sequences": props.filteredLogs
          .map((log: { sequence: number }) => log.sequence)
          .join(","),
        "data-selected-index": props.selectedIndex === null ? "null" : String(props.selectedIndex),
        "data-presentation": props.presentation,
      }),
  };
});

import { TooltipProvider } from "@/components/ui/tooltip";
import {
  DEFAULT_LOGS_DOCKVIEW_LAYOUT,
  LOGS_DOCKVIEW_COMPONENT_ID,
} from "@/features/core/dockview/logsDockviewLayout";
import { logsDockviewRuntime } from "@/features/core/dockview/logsRuntime";
import { logsDockviewRootBinding } from "@/features/core/dockview/logsRootBinding";
import { logDomainPanelId } from "@/features/domain/log/logDomains";
import { logBuffer } from "@/features/application/log/logBuffer";
import { useLogStore } from "@/features/application/log/logStore";
import { LogDomainDockviewHost, type LogDomainLayoutLifecycle } from "./LogDomainDockviewHost";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

type FakeDockviewInstance = {
  readonly api: DockviewApi;
  readonly props: Record<string, unknown>;
  readonly fromJSONInputs: Array<Record<string, any>>;
};

function diagnostic(domain: DiagnosticRecordDto["domain"], sequence: number): DiagnosticRecordDto {
  return {
    streamId: "stream-1",
    sequence,
    timestamp: "2026-08-22T10:00:00.000Z",
    level: "info",
    origin: "rust",
    domain,
    target: `${domain}.target`,
    message: `${domain} message`,
    fields: {},
  };
}

function latestDockview(): FakeDockviewInstance {
  const instance = mocks.dockviews[mocks.dockviews.length - 1];
  if (!instance) throw new Error("expected a fake Dockview instance");
  return instance as FakeDockviewInstance;
}

function panelList(group: Element): HTMLElement {
  const list = group.querySelector<HTMLElement>("[data-log-panel-list]");
  if (!list) throw new Error("expected an active Log domain panel");
  return list;
}

describe("LogDomainDockviewHost", () => {
  let host: HTMLDivElement;
  let root: Root | null;

  beforeEach(() => {
    mocks.dockviews.length = 0;
    mocks.entries.length = 0;
    vi.clearAllMocks();
    logBuffer.clear();
    useLogStore.setState({
      filter: {
        levels: new Set(["trace", "debug", "info", "warn", "error"]),
        searchText: "",
      },
      selectedLog: null,
      autoScroll: true,
    });
    mocks.subscribeDiagnostics.mockImplementation(async () => {
      const latestEntry = mocks.entries[mocks.entries.length - 1];
      return {
        snapshot: {
          subscriptionId: "logs-workspace-test",
          streamId: "stream-1",
          entries: [...mocks.entries],
          latestSequence: latestEntry?.sequence ?? 0,
          truncated: false,
        },
        activate: mocks.activateSubscription,
        unsubscribe: mocks.unsubscribeDiagnostics,
      };
    });
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    if (root) act(() => root?.unmount());
    root = null;
    document.body.replaceChildren();
    vi.restoreAllMocks();
  });

  const renderWorkspace = (layout: LogDomainLayoutLifecycle) => {
    act(() => {
      root?.render(
        <TooltipProvider>
          <LogDomainDockviewHost layout={layout} />
        </TooltipProvider>,
      );
    });
  };

  const flushSubscription = async () => {
    await act(async () => {
      await Promise.resolve();
    });
  };

  it("mounts one diagnostics subscription and seven native bounded tabs", async () => {
    renderWorkspace({ kind: "ephemeral" });
    await flushSubscription();

    expect(mocks.subscribeDiagnostics).toHaveBeenCalledOnce();
    expect(host.querySelectorAll('[role="tab"]')).toHaveLength(7);
    expect(host.querySelector('[role="tab"]')?.textContent).toBe("log.domains.all");
    expect(host.querySelector("[data-yssbi-logs-dockview]")?.className).toBe(
      "h-full min-h-0 w-full min-w-0",
    );

    const props = latestDockview().props;
    expect(Object.keys(props.components as object)).toEqual([LOGS_DOCKVIEW_COMPONENT_ID]);
    expect(props.defaultTabComponent).toBeTypeOf("function");
    expect(props.disableFloatingGroups).toBe(true);
    expect(props.theme).toMatchObject({
      name: "yssbi-logs-dark",
      edgeGroupCollapsedSize: 30,
    });
    expect(props).not.toHaveProperty("onUnhandledDragOver");
    expect(props.onWillDrop).toBeTypeOf("function");

    expect(host.querySelectorAll("[data-yssbi-logs-tab]")).toHaveLength(7);
    expect(host.querySelectorAll("[data-yssbi-logs-tab] button")).toHaveLength(0);
    expect(host.querySelector('button[aria-label="log.openDomain"]')).toBeNull();

    expect(host.querySelector('button[aria-label="log.refresh"]')).not.toBeNull();
    expect(host.querySelector('button[aria-label="log.autoScrollEnabled"]')).not.toBeNull();
    expect(host.querySelector('button[aria-label="log.clear"]')).not.toBeNull();
    const filterButton = host.querySelector<HTMLButtonElement>('button[aria-label="log.filter"]');
    act(() => filterButton?.click());
    expect(document.querySelector('input[aria-label="log.searchPlaceholder"]')).not.toBeNull();
    const levelButtons = [
      ...document.querySelectorAll<HTMLButtonElement>('[data-slot="popover-content"] button'),
    ].filter((button) =>
      ["TRACE", "DEBUG", "INFO", "WARN", "ERROR"].includes(button.textContent ?? ""),
    );
    expect(levelButtons).toHaveLength(5);
    expect(levelButtons.every((button) => button.getAttribute("aria-pressed") === "true")).toBe(
      true,
    );
  });

  it("updates a logs tab when Dockview changes its title", async () => {
    renderWorkspace({ kind: "ephemeral" });
    await flushSubscription();

    const panel = latestDockview().api.getPanel(logDomainPanelId("all"));
    if (!panel) throw new Error("expected the All log panel");

    act(() => panel.api.setTitle("Translated All"));

    expect(host.querySelector("[data-yssbi-logs-tab]")?.textContent).toBe("Translated All");
  });

  it("allows only same-group tab reordering drops", async () => {
    renderWorkspace({ kind: "ephemeral" });
    await flushSubscription();
    const props = latestDockview().props;
    const onWillDrop = props.onWillDrop as (event: {
      kind: string;
      group?: { id: string };
      getData: () => { groupId: string; panelId: string | null } | undefined;
      preventDefault: () => void;
    }) => void;
    const sameGroupTab = {
      kind: "tab",
      group: { id: "logs-domain-group" },
      getData: () => ({ groupId: "logs-domain-group", panelId: logDomainPanelId("all") }),
      preventDefault: vi.fn(),
    };
    const crossGroupTab = {
      ...sameGroupTab,
      group: { id: "logs-other-group" },
      preventDefault: vi.fn(),
    };
    const contentDrop = {
      ...sameGroupTab,
      kind: "content",
      preventDefault: vi.fn(),
    };

    act(() => {
      onWillDrop(sameGroupTab);
      onWillDrop(crossGroupTab);
      onWillDrop(contentDrop);
    });

    expect(sameGroupTab.preventDefault).not.toHaveBeenCalled();
    expect(crossGroupTab.preventDefault).toHaveBeenCalledOnce();
    expect(contentDrop.preventDefault).toHaveBeenCalledOnce();
  });

  it("binds the main root and unbinds its exact binding token", () => {
    const bind = vi.spyOn(logsDockviewRootBinding, "bind");
    const unbind = vi.spyOn(logsDockviewRootBinding, "unbind");
    renderWorkspace({ kind: "main" });
    const dockview = latestDockview();

    expect(bind).toHaveBeenCalledOnce();
    expect(bind).toHaveBeenCalledWith(dockview.api);
    act(() => root?.unmount());
    root = null;

    expect(unbind).toHaveBeenCalledOnce();
    expect(unbind).toHaveBeenCalledWith(bind.mock.results[0]?.value);
    expect(logsDockviewRuntime.getLatestSnapshot().panels).toHaveProperty(logDomainPanelId("ui"));
  });

  it("restores a fresh ephemeral default without controller or storage persistence", async () => {
    const setItem = vi.spyOn(Storage.prototype, "setItem");
    const bind = vi.spyOn(logsDockviewRootBinding, "bind");
    const unbind = vi.spyOn(logsDockviewRootBinding, "unbind");

    renderWorkspace({ kind: "ephemeral" });
    await flushSubscription();
    const dockview = latestDockview();

    expect(dockview.fromJSONInputs).toHaveLength(1);
    expect(dockview.fromJSONInputs[0]).toEqual(DEFAULT_LOGS_DOCKVIEW_LAYOUT);
    expect(dockview.fromJSONInputs[0]).not.toBe(DEFAULT_LOGS_DOCKVIEW_LAYOUT);
    expect(bind).not.toHaveBeenCalled();
    expect(unbind).not.toHaveBeenCalled();
    expect(setItem).not.toHaveBeenCalled();
  });

  it("projects selection independently in split All and Graph panels", async () => {
    const applicationLog = diagnostic("application", 1);
    const selectedGraphLog = diagnostic("graph", 2);
    const otherGraphLog = diagnostic("graph", 3);
    mocks.entries.push(applicationLog, selectedGraphLog, otherGraphLog);
    useLogStore.setState({ selectedLog: selectedGraphLog });
    renderWorkspace({ kind: "ephemeral" });
    await flushSubscription();

    const dockview = latestDockview();
    act(() => {
      const splitGroup = dockview.api.addGroup({
        id: "logs-split-right",
        referenceGroup: dockview.api.groups[0],
        direction: "right",
      });
      dockview.api.getPanel(logDomainPanelId("graph"))?.api.moveTo({ group: splitGroup });
    });

    const allGroup = host.querySelector('[data-group-id="logs-domain-group"]');
    const graphGroup = host.querySelector('[data-group-id="logs-split-right"]');
    if (!allGroup || !graphGroup) throw new Error("expected both split Log groups");

    expect(panelList(allGroup).dataset.sequences).toBe("1,2,3");
    expect(panelList(allGroup).dataset.selectedIndex).toBe("1");
    expect(panelList(graphGroup).dataset.sequences).toBe("2,3");
    expect(panelList(graphGroup).dataset.selectedIndex).toBe("0");
    expect(panelList(graphGroup).dataset.presentation).toBe("standalone");
  });
});
