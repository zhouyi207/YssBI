import type {
  DockviewApi,
  IDockviewGroupPanel,
  IDockviewPanel,
} from 'dockview-react';

import { sanitizeDockviewLayout } from './sanitizeDockviewLayout';
import type {
  DockviewGroupInfo,
  DockviewLayout,
  DockviewPanelInfo,
  DockviewPanelParams,
  DockviewPortSnapshot,
  LayoutTabMetadata,
  MovePanelRequest,
  OpenPanelRequest,
  PanelInstanceId,
  ResourceRef,
  SplitPanelRequest,
} from './types';

interface Disposable {
  dispose(): void;
}

interface PendingCommand {
  run(api: DockviewApi): void;
}

export interface DockviewEditorPort {
  bind(api: DockviewApi): void;
  unbind(api?: DockviewApi): void;
  readonly isReady: boolean;
  subscribe(listener: () => void): () => void;
  getSnapshot(): DockviewPortSnapshot;
  getActiveGroupId(): string | undefined;
  getActivePanel(): DockviewPanelInfo | undefined;
  findPanelsByResource(resourceRef: ResourceRef): readonly DockviewPanelInfo[];
  listGroups(): readonly DockviewGroupInfo[];
  listPanels(): readonly DockviewPanelInfo[];
  open(request: OpenPanelRequest): Promise<DockviewPanelInfo>;
  activate(panelInstanceId: PanelInstanceId): Promise<boolean>;
  updateTab(panelInstanceId: PanelInstanceId, tab: LayoutTabMetadata): Promise<boolean>;
  remove(panelInstanceId: PanelInstanceId): Promise<boolean>;
  move(request: MovePanelRequest): Promise<boolean>;
  split(request: SplitPanelRequest): Promise<boolean>;
  serialize(): Promise<DockviewLayout>;
  restore(layout: DockviewLayout): Promise<void>;
  reset(): Promise<void>;
  remapResource(from: ResourceRef, to: ResourceRef): Promise<number>;
}

function getTab(panel: IDockviewPanel): LayoutTabMetadata | undefined {
  const params = panel.params as Partial<DockviewPanelParams> | undefined;
  return params?.layoutTab;
}

function toPanelInfo(panel: IDockviewPanel): DockviewPanelInfo {
  return {
    panelInstanceId: panel.id,
    groupId: panel.group.id,
    component: panel.api.component,
    title: panel.title,
    tab: getTab(panel),
    active: panel.api.isActive,
  };
}

function toGroupInfo(
  group: IDockviewGroupPanel,
  activeGroupId: string | undefined,
): DockviewGroupInfo {
  return {
    groupId: group.id,
    panelInstanceIds: group.panels.map((panel) => panel.id),
    activePanelInstanceId: group.activePanel?.id,
    active: group.id === activeGroupId,
  };
}

export function createDockviewEditorPort(): DockviewEditorPort {
  let api: DockviewApi | undefined;
  let revision = 0;
  let snapshot: DockviewPortSnapshot = Object.freeze({ revision, ready: false });
  let eventDisposables: Disposable[] = [];
  const listeners = new Set<() => void>();
  const pending: PendingCommand[] = [];

  const publish = (): void => {
    revision += 1;
    snapshot = Object.freeze({ revision, ready: api !== undefined });
    listeners.forEach((listener) => listener());
  };

  const execute = <T>(operation: (boundApi: DockviewApi) => T): Promise<T> => {
    if (api) {
      try {
        return Promise.resolve(operation(api));
      } catch (error) {
        return Promise.reject(error);
      }
    }

    return new Promise<T>((resolve, reject) => {
      pending.push({
        run(boundApi) {
          try {
            resolve(operation(boundApi));
          } catch (error) {
            reject(error);
          }
        },
      });
    });
  };

  const requirePanel = (
    boundApi: DockviewApi,
    panelInstanceId: PanelInstanceId,
  ): IDockviewPanel | undefined => boundApi.getPanel(panelInstanceId);

  const port: DockviewEditorPort = {
    bind(boundApi) {
      if (api === boundApi) return;
      port.unbind();
      api = boundApi;
      const invalidate = (): void => publish();
      eventDisposables = [
        boundApi.onDidLayoutChange(invalidate),
        boundApi.onDidActiveGroupChange(invalidate),
        boundApi.onDidActivePanelChange(invalidate),
      ];
      publish();
      while (pending.length > 0 && api === boundApi) {
        pending.shift()?.run(boundApi);
      }
    },

    unbind(expectedApi) {
      if (!api || (expectedApi && expectedApi !== api)) return;
      eventDisposables.forEach((disposable) => disposable.dispose());
      eventDisposables = [];
      api = undefined;
      publish();
    },

    get isReady() {
      return api !== undefined;
    },

    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },

    getSnapshot() {
      return snapshot;
    },

    getActiveGroupId() {
      return api?.activeGroup?.id;
    },

    getActivePanel() {
      return api?.activePanel ? toPanelInfo(api.activePanel) : undefined;
    },

    findPanelsByResource(resourceRef) {
      return api?.panels
        .filter((panel) => getTab(panel)?.resourceRef === resourceRef)
        .map(toPanelInfo) ?? [];
    },

    listGroups() {
      const activeGroupId = api?.activeGroup?.id;
      return api?.groups.map((group) => toGroupInfo(group, activeGroupId)) ?? [];
    },

    listPanels() {
      return api?.panels.map(toPanelInfo) ?? [];
    },

    open(request) {
      return execute((boundApi) => {
        const group = request.groupId
          ? boundApi.getGroup(request.groupId)
          : undefined;
        if (request.groupId && !group) {
          throw new Error(`Dockview group not found: ${request.groupId}`);
        }
        const panel = boundApi.addPanel<DockviewPanelParams>({
          id: request.panelInstanceId,
          component: request.component,
          title: request.title,
          tabComponent: request.tabComponent,
          params: { ...request.params, layoutTab: request.tab },
          inactive: request.inactive,
          position: group
            ? { referenceGroup: group.id, direction: 'within', index: request.index }
            : undefined,
        });
        return toPanelInfo(panel);
      });
    },

    activate(panelInstanceId) {
      return execute((boundApi) => {
        const panel = requirePanel(boundApi, panelInstanceId);
        panel?.api.setActive();
        return panel !== undefined;
      });
    },

    updateTab(panelInstanceId, tab) {
      return execute((boundApi) => {
        const panel = requirePanel(boundApi, panelInstanceId);
        if (!panel) return false;
        panel.api.updateParameters({ ...(panel.params ?? {}), layoutTab: tab });
        return true;
      });
    },

    remove(panelInstanceId) {
      return execute((boundApi) => {
        const panel = requirePanel(boundApi, panelInstanceId);
        if (!panel) return false;
        panel.api.close();
        return true;
      });
    },

    move(request) {
      return execute((boundApi) => {
        const panel = requirePanel(boundApi, request.panelInstanceId);
        const group = boundApi.groups.find(({ id }) => id === request.groupId);
        if (!panel || !group) return false;
        panel.api.moveTo({
          group,
          position: 'center',
          index: request.index,
          skipSetActive: request.activate === false,
        });
        return true;
      });
    },

    split(request) {
      return execute((boundApi) => {
        const panel = requirePanel(boundApi, request.panelInstanceId);
        const group = boundApi.groups.find(
          ({ id }) => id === request.referenceGroupId,
        );
        if (!panel || !group) return false;
        panel.api.moveTo({
          group,
          position: request.direction,
          skipSetActive: request.activate === false,
        });
        return true;
      });
    },

    serialize() {
      return execute((boundApi) => boundApi.toJSON());
    },

    restore(layout) {
      const sanitized = sanitizeDockviewLayout(layout);
      return execute((boundApi) => boundApi.fromJSON(sanitized));
    },

    reset() {
      return execute((boundApi) => boundApi.clear());
    },

    remapResource(from, to) {
      return execute((boundApi) => {
        let remapped = 0;
        boundApi.panels.forEach((panel) => {
          const tab = getTab(panel);
          if (!tab || tab.resourceRef !== from) return;
          const nested = tab.data?.layoutTab;
          const remappedNested = nested && typeof nested === 'object' && 'id' in nested
            ? { ...nested, id: to }
            : nested;
          panel.api.updateParameters({
            ...(panel.params ?? {}),
            layoutTab: {
              ...tab,
              resourceRef: to,
              data: tab.data
                ? { ...tab.data, ...(remappedNested ? { layoutTab: remappedNested } : {}) }
                : tab.data,
            },
          });
          remapped += 1;
        });
        return remapped;
      });
    },
  };

  return port;
}

/** Application-wide editor dock. Dockview itself remains the topology authority. */
export const editorDockviewPort = createDockviewEditorPort();
