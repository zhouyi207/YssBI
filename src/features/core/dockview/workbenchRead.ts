import type { DeepReadonly } from '@/features/core/projection/deepReadonly';
import {
  workbenchDockviewPort,
  type WorkbenchDockviewPort,
  type WorkbenchEdgePosition,
  type WorkbenchEdgeState,
  type WorkbenchGroupInfo,
  type WorkbenchPanelInfo,
} from './workbenchDockviewPort';

export interface WorkbenchDockviewRead {
  readonly isReady: boolean;
  readonly isHydrated: boolean;
  whenHydrated(): Promise<{ readonly status: 'hydrated' | 'unbound' }>;
  subscribe(listener: () => void): () => void;
  getSnapshot(): DeepReadonly<{ revision: number; ready: boolean; hydrated: boolean }>;
  getPanel(panelInstanceId: string): DeepReadonly<WorkbenchPanelInfo> | undefined;
  getActivePanel(): DeepReadonly<WorkbenchPanelInfo> | undefined;
  getActiveEditorPanel(): DeepReadonly<WorkbenchPanelInfo> | undefined;
  listPanels(): readonly DeepReadonly<WorkbenchPanelInfo>[];
  listGroups(): readonly DeepReadonly<WorkbenchGroupInfo>[];
  listGroupPanels(groupId: string): readonly DeepReadonly<WorkbenchPanelInfo>[];
  findEditorPanelsByResource(resourceRef: string): readonly DeepReadonly<WorkbenchPanelInfo>[];
  getEdgeState(position: WorkbenchEdgePosition): DeepReadonly<WorkbenchEdgeState>;
}

let bindingGeneration = 0;
let waiters: Array<{
  readonly generation: number;
  readonly resolve: (value: { readonly status: 'hydrated' | 'unbound' }) => void;
}> = [];

function settle(status: 'hydrated' | 'unbound', generation: number): void {
  const pending = waiters;
  waiters = [];
  for (const waiter of pending) {
    if (waiter.generation === generation || status === 'unbound') waiter.resolve({ status });
    else waiters.push(waiter);
  }
}

export function notifyWorkbenchRootBound(): void {
  bindingGeneration += 1;
  if (workbenchDockviewPort.isHydrated) settle('hydrated', bindingGeneration);
}

export function notifyWorkbenchRootUnbound(generation: number): void {
  settle('unbound', generation);
}

export function createWorkbenchDockviewRead(
  port: WorkbenchDockviewPort = workbenchDockviewPort,
): WorkbenchDockviewRead {
  return {
    get isReady() { return port.isReady; },
    get isHydrated() { return port.isHydrated; },
    whenHydrated: () => {
      const generation = bindingGeneration;
      if (!port.isHydrated) {
        return new Promise((resolve) => waiters.push({ generation, resolve }));
      }
      return Promise.resolve({ status: 'hydrated' as const });
    },
    subscribe: port.subscribe,
    getSnapshot: port.getSnapshot,
    getPanel: port.getPanel,
    getActivePanel: port.getActivePanel,
    getActiveEditorPanel: port.getActiveEditorPanel,
    listPanels: port.listPanels,
    listGroups: port.listGroups,
    listGroupPanels: port.listGroupPanels,
    findEditorPanelsByResource: port.findEditorPanelsByResource,
    getEdgeState: port.getEdgeState,
  };
}

export const workbenchDockviewRead = createWorkbenchDockviewRead();

export type {
  WorkbenchEdgePosition,
  WorkbenchEdgeState,
  WorkbenchGroupInfo,
  WorkbenchPanelInfo,
};
