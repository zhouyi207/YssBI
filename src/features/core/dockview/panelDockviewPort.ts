import type {
  DockviewApi,
  DockviewGroupPanelApi,
} from 'dockview-react';

import { WORKBENCH_PANEL_COLLAPSED_HEIGHT } from '@/features/core/workbench';
import { sanitizeDockviewLayout } from './sanitizeDockviewLayout';
import type { DockviewLayout } from './types';

interface Disposable {
  dispose(): void;
}

interface PendingCommand {
  run(api: DockviewApi): void;
}

export type PanelDockPosition = 'bottom' | 'left' | 'right';

const PANEL_EDGE_POSITIONS: readonly PanelDockPosition[] = ['bottom', 'left', 'right'];
const PANEL_EDGE_GROUP_ID_PREFIX = 'workbench-panel';

export interface PanelDockviewPort {
  bind(api: DockviewApi): void;
  unbind(api?: DockviewApi): void;
  readonly isReady: boolean;
  whenReady(): Promise<void>;
  subscribe(listener: () => void): () => void;
  activate(panelId: string): Promise<boolean>;
  getPosition(): PanelDockPosition | undefined;
  isCollapsed(): boolean | undefined;
  setCollapsed(collapsed: boolean): Promise<boolean>;
  setPosition(position: PanelDockPosition): Promise<boolean>;
  serialize(): Promise<DockviewLayout>;
  restore(layout: DockviewLayout): Promise<void>;
  reset(): Promise<void>;
}

function cloneLayout(layout: DockviewLayout): DockviewLayout {
  return structuredClone(layout);
}

function edgePosition(api: DockviewApi): PanelDockPosition | undefined {
  return PANEL_EDGE_POSITIONS.find((position) => api.getEdgeGroup(position) !== undefined);
}

function edgeGroup(api: DockviewApi): DockviewGroupPanelApi | undefined {
  const position = edgePosition(api);
  return position ? api.getEdgeGroup(position) : undefined;
}

function serializedExpandedSize(
  api: DockviewApi,
  position: PanelDockPosition,
): number | undefined {
  return api.toJSON().edgeGroups?.[position]?.size;
}

function moveEdgeGroup(api: DockviewApi, position: PanelDockPosition): boolean {
  const currentPosition = edgePosition(api);
  if (!currentPosition || currentPosition === position) return currentPosition === position;

  const currentGroupApi = api.getEdgeGroup(currentPosition);
  const currentGroup = currentGroupApi
    ? api.groups.find((group) => group.id === currentGroupApi.id)
    : undefined;
  if (!currentGroupApi || !currentGroup) return false;

  const collapsed = currentGroupApi.isCollapsed();
  const expandedSize = serializedExpandedSize(api, currentPosition);
  const activePanelId = currentGroup.activePanel?.id;
  const panels = [...currentGroup.panels];
  const targetGroupApi = api.addEdgeGroup(position, {
    id: `${PANEL_EDGE_GROUP_ID_PREFIX}-${position}`,
    initialSize: expandedSize,
    minimumSize: WORKBENCH_PANEL_COLLAPSED_HEIGHT,
    collapsedSize: WORKBENCH_PANEL_COLLAPSED_HEIGHT,
    collapsed,
  });
  const targetGroup = api.groups.find((group) => group.id === targetGroupApi.id);
  if (!targetGroup) {
    api.removeEdgeGroup(position);
    return false;
  }

  panels.forEach((panel, index) => {
    panel.api.moveTo({
      group: targetGroup,
      position: 'center',
      index,
      skipSetActive: true,
    });
  });
  api.removeEdgeGroup(currentPosition);
  if (activePanelId) api.getPanel(activePanelId)?.api.setActive();
  if (collapsed) targetGroupApi.collapse();
  return true;
}

export function createPanelDockviewPort(): PanelDockviewPort {
  let api: DockviewApi | undefined;
  let defaultLayout: DockviewLayout | undefined;
  let eventDisposables: Disposable[] = [];
  let collapsedDisposable: Disposable | undefined;
  const listeners = new Set<() => void>();
  const pending: PendingCommand[] = [];

  const publish = (): void => {
    listeners.forEach((listener) => listener());
  };

  const bindCollapsedEvent = (): void => {
    collapsedDisposable?.dispose();
    collapsedDisposable = api
      ? edgeGroup(api)?.onDidCollapsedChange(publish)
      : undefined;
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

  const port: PanelDockviewPort = {
    bind(boundApi) {
      if (api === boundApi) return;
      port.unbind();
      api = boundApi;
      defaultLayout = cloneLayout(boundApi.toJSON());
      eventDisposables = [
        boundApi.onDidLayoutChange(publish),
        boundApi.onDidActivePanelChange(publish),
        boundApi.onDidLayoutFromJSON(() => {
          bindCollapsedEvent();
          publish();
        }),
      ];
      bindCollapsedEvent();
      publish();
      while (pending.length > 0 && api === boundApi) {
        pending.shift()?.run(boundApi);
      }
    },

    unbind(expectedApi) {
      if (!api || (expectedApi && expectedApi !== api)) return;
      eventDisposables.forEach((disposable) => disposable.dispose());
      eventDisposables = [];
      collapsedDisposable?.dispose();
      collapsedDisposable = undefined;
      api = undefined;
      defaultLayout = undefined;
      publish();
    },

    get isReady() {
      return api !== undefined;
    },

    whenReady() {
      if (api) return Promise.resolve();
      return new Promise<void>((resolve) => {
        const unsubscribe = port.subscribe(() => {
          if (!port.isReady) return;
          unsubscribe();
          resolve();
        });
      });
    },

    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },

    activate(panelId) {
      return execute((boundApi) => {
        const panel = boundApi.getPanel(panelId);
        panel?.api.setActive();
        return panel !== undefined;
      });
    },

    getPosition() {
      return api ? edgePosition(api) : undefined;
    },

    isCollapsed() {
      return api ? edgeGroup(api)?.isCollapsed() : undefined;
    },

    setCollapsed(collapsed) {
      return execute((boundApi) => {
        const group = edgeGroup(boundApi);
        if (!group) return false;
        if (collapsed) group.collapse();
        else group.expand();
        return true;
      });
    },


    setPosition(position) {
      return execute((boundApi) => {
        const moved = moveEdgeGroup(boundApi, position);
        if (moved) {
          bindCollapsedEvent();
          publish();
        }
        return moved;
      });
    },

    serialize() {
      return execute((boundApi) => boundApi.toJSON());
    },

    restore(layout) {
      const sanitized = sanitizeDockviewLayout(cloneLayout(layout));
      return execute((boundApi) => boundApi.fromJSON(sanitized));
    },

    reset() {
      return execute((boundApi) => {
        if (defaultLayout) boundApi.fromJSON(cloneLayout(defaultLayout));
        else boundApi.clear();
      });
    },
  };

  return port;
}

/** Dockview shell authority for the editor host and Logs/Output edge group. */
export const panelDockviewPort = createPanelDockviewPort();
