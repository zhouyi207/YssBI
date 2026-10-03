import { createStore } from "zustand/vanilla";
import { subscribeWithSelector } from "zustand/middleware";
import { shallow } from "zustand/shallow";
import { castDraft, produce } from "immer";
import { freezePublishedValue } from "@/shared/types/deepReadonly";
import type { WorkbenchModelOperations } from "./workbenchLayoutOperations";
import type {
  WorkbenchLayoutReadContract,
  WorkbenchEditorPanelInfo,
  WorkbenchPanelInfo,
  WorkbenchGroupInfo,
  WorkbenchEdgeState,
  WorkbenchEdgePosition,
} from "./workbenchTypes";

interface LayoutProjection {
  readonly panels: Readonly<Record<string, WorkbenchPanelInfo>>;
  readonly groups: readonly WorkbenchGroupInfo[];
  readonly edges: Readonly<Record<WorkbenchEdgePosition, WorkbenchEdgeState>>;
}

type LayoutSnapshot = ReturnType<WorkbenchLayoutReadContract["getSnapshot"]>;
interface PublishedLayout {
  readonly projection: LayoutProjection;
  readonly activePanel: WorkbenchPanelInfo | undefined;
  readonly snapshot: LayoutSnapshot;
  readonly activeSnapshot: LayoutSnapshot;
  readonly panelSetSnapshot: LayoutSnapshot;
}

type ProjectionRead = Omit<
  WorkbenchLayoutReadContract,
  "isReady" | "isHydrated" | "getMutationRevision"
>;

const positions = ["left", "right", "top", "bottom"] as const;

function observe(listener: () => void): () => void {
  return () => {
    try {
      listener();
    } catch {
      // Observers cannot interrupt a committed layout change or other subscribers.
    }
  };
}

/** Publishes read records from native Model commits; only the Model accepts layout writes. */
export function createWorkbenchLayoutProjection() {
  const snapshot: LayoutSnapshot = { revision: 0, ready: false, hydrated: false };
  const published = createStore<PublishedLayout>()(
    subscribeWithSelector((): PublishedLayout => ({
      projection: {
        panels: {},
        groups: [],
        edges: Object.fromEntries(
          positions.map((position) => [
            position,
            { position, exists: false, visible: false, collapsed: true },
          ]),
        ) as Record<WorkbenchEdgePosition, WorkbenchEdgeState>,
      },
      activePanel: undefined,
      snapshot,
      activeSnapshot: snapshot,
      panelSetSnapshot: snapshot,
    })),
  );

  const publish = (model: WorkbenchModelOperations | undefined, hydrated: boolean): void => {
    const previous = published.getState();
    const next = produce(previous.projection, (draft) => {
      const panelIds = new Set<string>();
      for (const panel of model?.listPanels() ?? []) {
        panelIds.add(panel.panelInstanceId);
        const before = previous.projection.panels[panel.panelInstanceId];
        let metadata = panel.metadata;
        if (
          metadata !== before?.metadata &&
          metadata.role === "result" &&
          before?.metadata.role === "result"
        ) {
          metadata = {
            ...metadata,
            reference: shallow(before.metadata.reference, metadata.reference)
              ? before.metadata.reference
              : metadata.reference,
            presentation: shallow(before.metadata.presentation, metadata.presentation)
              ? before.metadata.presentation
              : metadata.presentation,
          };
        }
        const candidate = {
          ...panel,
          metadata: before && shallow(before.metadata, metadata) ? before.metadata : metadata,
          location:
            before && shallow(before.location, panel.location) ? before.location : panel.location,
        };
        if (!shallow(before, candidate)) draft.panels[panel.panelInstanceId] = castDraft(candidate);
      }
      for (const id in draft.panels) if (!panelIds.has(id)) delete draft.panels[id];

      const previousGroups = new Map(
        previous.projection.groups.map((group) => [group.groupId, group]),
      );
      const groups = (model?.listGroups() ?? []).map((group) => {
        const before = previousGroups.get(group.groupId);
        const candidate = {
          ...group,
          panelInstanceIds:
            before && shallow(before.panelInstanceIds, group.panelInstanceIds)
              ? before.panelInstanceIds
              : group.panelInstanceIds,
          location:
            before && shallow(before.location, group.location) ? before.location : group.location,
        };
        return before && shallow(before, candidate) ? before : candidate;
      });
      if (!shallow(previous.projection.groups, groups)) draft.groups = castDraft(groups);
      for (const position of positions) {
        const edge = model?.getEdgeState(position) ?? {
          position,
          exists: false,
          visible: false,
          collapsed: true,
        };
        if (!shallow(previous.projection.edges[position], edge)) draft.edges[position] = edge;
      }
    });
    const ready = Boolean(model);
    const lifecycleChanged =
      previous.snapshot.ready !== ready || previous.snapshot.hydrated !== hydrated;
    const activePanel = Object.values(next.panels).find((panel) => panel.active);
    const semanticChanged = lifecycleChanged || next !== previous.projection;
    const activeChanged = lifecycleChanged || activePanel !== previous.activePanel;
    const panelSetChanged =
      lifecycleChanged ||
      Object.keys(next.panels).length !== Object.keys(previous.projection.panels).length ||
      Object.values(next.panels).some(
        (panel) => previous.projection.panels[panel.panelInstanceId]?.metadata !== panel.metadata,
      );
    const snapshot = semanticChanged
      ? { revision: previous.snapshot.revision + 1, ready, hydrated }
      : previous.snapshot;
    // Every commit schedules persistence. Selectors suppress unrelated business notifications.
    published.setState(
      freezePublishedValue({
        projection: next,
        activePanel,
        snapshot,
        activeSnapshot: activeChanged ? snapshot : previous.activeSnapshot,
        panelSetSnapshot: panelSetChanged ? snapshot : previous.panelSetSnapshot,
      }),
      true,
    );
  };

  const read: ProjectionRead = {
    subscribe: (listener) => published.subscribe((state) => state.snapshot, observe(listener)),
    subscribePersistence: (listener) => published.subscribe(observe(listener)),
    subscribeActivePanel: (listener) =>
      published.subscribe((state) => state.activeSnapshot, observe(listener)),
    subscribePanelSet: (listener) =>
      published.subscribe((state) => state.panelSetSnapshot, observe(listener)),
    subscribePanel: (id, listener) =>
      published.subscribe(
        ({ projection, snapshot }) => {
          const panel = projection.panels[id];
          return [
            snapshot.ready,
            snapshot.hydrated,
            panel,
            panel?.location.type === "edge" ? projection.edges[panel.location.position] : undefined,
          ];
        },
        observe(listener),
        { equalityFn: shallow },
      ),
    getSnapshot: () => published.getState().snapshot,
    getActiveSnapshot: () => published.getState().activeSnapshot,
    getPanel: (id) => published.getState().projection.panels[id],
    getActivePanel: () => published.getState().activePanel,
    getActiveEditorPanel: () => {
      const panel = published.getState().activePanel;
      return panel?.metadata.role === "editor" ? (panel as WorkbenchEditorPanelInfo) : undefined;
    },
    getActiveEditorPanelInGroup: (id) => {
      const { projection } = published.getState();
      const group = projection.groups.find((entry) => entry.groupId === id);
      const panel = group?.activePanelInstanceId
        ? projection.panels[group.activePanelInstanceId]
        : undefined;
      return panel?.metadata.role === "editor" ? (panel as WorkbenchEditorPanelInfo) : undefined;
    },
    listPanels: () => Object.values(published.getState().projection.panels),
    listGroups: () => published.getState().projection.groups,
    listGroupPanels: (id) =>
      Object.values(published.getState().projection.panels).filter((panel) => panel.groupId === id),
    listEditorPanelsInGroup: (id) =>
      Object.values(published.getState().projection.panels).filter(
        (panel): panel is WorkbenchEditorPanelInfo =>
          panel.groupId === id && panel.metadata.role === "editor",
      ),
    findEditorPanelsByResource: (resourceRef) =>
      Object.values(published.getState().projection.panels).filter(
        (panel): panel is WorkbenchEditorPanelInfo =>
          panel.metadata.role === "editor" && panel.metadata.resourceRef === resourceRef,
      ),
    getEdgeState: (position) => published.getState().projection.edges[position],
  };
  return { publish, read };
}
