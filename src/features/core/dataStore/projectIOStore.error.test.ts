import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ProjectService } from '@/services/project/projectService';
import { normalizeIpcError } from '@/services/ipc';
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import { LoadStatus } from '@/shared/types/ui/common';
import {
  registerProjectIOApplicationPort,
  resetProjectIOApplicationPort,
} from './projectIOApplicationPort';
import { useProjectIOStore } from './projectIOStore';

const loggerMocks = vi.hoisted(() => ({
  error: vi.fn(),
  info: vi.fn(),
}));

const loadGraphProjection = vi.hoisted(() => vi.fn<() => Promise<boolean>>());

vi.mock('@/utils/appLogger', () => ({
  logger: {
    sys: loggerMocks,
  },
}));

vi.mock('@/services/project/projectService', () => ({
  ProjectService: {
    getProjectPath: vi.fn(),
    getDatabasesVariables: vi.fn(),
    getProjectIndex: vi.fn(),
  },
}));

const projectInstanceId = 'project-error-state-test';

function installProjectIOPort(): void {
  registerProjectIOApplicationPort({
    hydrateFunctionSignatures: () => undefined,
    resetFunctionSignatures: () => undefined,
    resetHistory: () => undefined,
    cancelPublication: () => undefined,
    validatePublicationStart: () => undefined,
    startPublication: () => undefined,
    acceptProjectActivation: () => true,
    reconcileOpenTabs: () => undefined,
    resetGraphProjection: () => undefined,
    beginGraphLoad: () => 1,
    loadGraphProjection,
    submitPublication: async () => undefined,
  });
}

describe('projectIOStore error references', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    startProjectLifecycle(projectInstanceId);
    installProjectIOPort();
    useProjectIOStore.setState({
      status: LoadStatus.Idle,
      error: null,
      graphLoadStatus: {},
      currentPath: null,
      projectInstanceId,
    });
    vi.mocked(ProjectService.getProjectPath).mockResolvedValue(null);
    vi.mocked(ProjectService.getDatabasesVariables).mockResolvedValue({
      databases: {},
      variables: {},
    });
  });

  afterEach(() => {
    resetProjectIOApplicationPort();
    clearProjectLifecycle();
  });

  it('maps project parser prose to an explicit contract code', async () => {
    vi.mocked(ProjectService.getProjectIndex).mockRejectedValue(
      new Error('private project index parser prose'),
    );

    await expect(useProjectIOStore.getState().loadProject()).resolves.toBeNull();

    const state = useProjectIOStore.getState();
    expect(state.status).toBe(LoadStatus.Error);
    expect(state.error).toEqual({
      code: 'project_load_contract_error',
      incidentId: null,
    });
    expect(JSON.stringify(state.error)).not.toContain('private project index parser prose');
  });

  it('keeps only normalized transport code and drops transport prose', async () => {
    vi.mocked(ProjectService.getProjectPath).mockRejectedValue(
      normalizeIpcError('get_project_path', new Error('private project transport prose')),
    );

    await expect(useProjectIOStore.getState().loadProject()).resolves.toBeNull();

    expect(useProjectIOStore.getState().error).toEqual({
      code: 'ipc_transport_failure',
      incidentId: null,
    });
    expect(JSON.stringify(useProjectIOStore.getState().error)).not.toContain(
      'private project transport prose',
    );
  });

  it('preserves backend code and incident ID without retaining details', async () => {
    vi.mocked(ProjectService.getProjectPath).mockRejectedValue(
      normalizeIpcError('get_project_path', {
        code: 'project_io_failed',
        details: { debug: 'private project backend detail' },
        incidentId: 'incident-project-load-42',
      }),
    );

    await expect(useProjectIOStore.getState().loadProject()).resolves.toBeNull();

    expect(useProjectIOStore.getState().error).toEqual({
      code: 'project_io_failed',
      incidentId: 'incident-project-load-42',
    });
    expect(JSON.stringify(useProjectIOStore.getState().error)).not.toContain(
      'private project backend detail',
    );
  });

  it('maps resource-index parser prose to its stable contract code', async () => {
    vi.mocked(ProjectService.getProjectIndex).mockRejectedValue(
      new Error('private resource index parser prose'),
    );

    await expect(useProjectIOStore.getState().refreshResourceIndex()).resolves.toBe(false);

    expect(useProjectIOStore.getState().error).toEqual({
      code: 'project_resource_index_contract_error',
      incidentId: null,
    });
    expect(JSON.stringify(useProjectIOStore.getState().error)).not.toContain(
      'private resource index parser prose',
    );
  });

  it('maps graph projection rejections to a stable contract code', async () => {
    loadGraphProjection.mockRejectedValue(new Error('private graph projection prose'));

    await expect(
      useProjectIOStore.getState().loadGraph('events/ErrorStateMigration.yssbi-event'),
    ).resolves.toBe(false);

    expect(useProjectIOStore.getState().error).toEqual({
      code: 'graph_projection_contract_error',
      incidentId: null,
    });
    expect(JSON.stringify(useProjectIOStore.getState().error)).not.toContain(
      'private graph projection prose',
    );
  });
});
