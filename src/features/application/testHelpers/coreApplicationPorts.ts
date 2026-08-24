import {
  registerProjectIOApplicationPort,
  resetProjectIOApplicationPort,
  type ProjectIOApplicationPort,
} from '@/features/core/dataStore/projectIOApplicationPort';
import {
  registerPendingMutationPort,
  resetPendingMutationPort,
  type PendingMutationPort,
} from '@/features/core/history/pendingMutationPort';
import {
  registerSyncApplicationEventPort,
  resetSyncApplicationEventPort,
  type SyncApplicationEventPort,
} from '@/features/core/sync/applicationEventPort';
import {
  registerWorksheetApplicationPort,
  resetWorksheetApplicationPort,
  type WorksheetApplicationPort,
} from '@/features/core/worksheet/worksheetApplicationPort';

export interface CoreApplicationTestPorts {
  projectIO: ProjectIOApplicationPort;
  pendingMutation: PendingMutationPort;
  syncEvents: SyncApplicationEventPort;
  worksheet: WorksheetApplicationPort;
}

type CoreApplicationTestPortOverrides = {
  [Key in keyof CoreApplicationTestPorts]?: Partial<CoreApplicationTestPorts[Key]>;
};

export function installCoreApplicationTestPorts(
  overrides: CoreApplicationTestPortOverrides = {},
): CoreApplicationTestPorts {
  const ports: CoreApplicationTestPorts = {
    projectIO: {
      hydrateFunctionSignatures: () => undefined,
      resetFunctionSignatures: () => undefined,
      resetHistory: () => undefined,
      validatePublicationStart: () => undefined,
      startPublication: () => undefined,
      acceptProjectActivation: () => true,
      reconcileOpenTabs: () => undefined,
      removeProjectScopedWorkbenchPanels: async () => undefined,
      resetGraphProjection: () => undefined,
      beginGraphLoad: () => 0,
      loadGraphProjection: async () => false,
      submitPublication: async () => undefined,
      ...overrides.projectIO,
    },
    pendingMutation: {
      graphPathFor: () => undefined,
      ...overrides.pendingMutation,
    },
    syncEvents: {
      graphDelta: () => undefined,
      computationSettingsChanged: () => undefined,
      resourceMutationCommitted: async () => undefined,
      applyProjectLifecycleReceipt: async () => undefined,
      clearProject: async () => undefined,
      ...overrides.syncEvents,
    },
    worksheet: {
      captureCommandContext: () => ({
        projectInstanceId: 'test-project',
        operationId: 'test-operation',
        isCurrent: () => true,
      }),
      submitPublication: async () => undefined,
      ...overrides.worksheet,
    },
  };

  registerProjectIOApplicationPort(ports.projectIO);
  registerPendingMutationPort(ports.pendingMutation);
  registerSyncApplicationEventPort(ports.syncEvents);
  registerWorksheetApplicationPort(ports.worksheet);
  return ports;
}

export function resetCoreApplicationTestPorts(): void {
  resetProjectIOApplicationPort();
  resetPendingMutationPort();
  resetSyncApplicationEventPort();
  resetWorksheetApplicationPort();
}
