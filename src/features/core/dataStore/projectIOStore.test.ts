import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ProjectData } from '@/shared/types';
import type { GraphData } from '@/shared/types/store/graph';
import { graphDataToDomainGraph } from '@/shared/types/dto/graphModel';
import { LoadStatus } from '@/shared/types/ui/common';
import { useDatabaseStore } from './databaseStore';
import { useGraphDataStore } from './graphDataStore';
import { useProjectIOStore } from './projectIOStore';
import { useResourceStore } from '@/features/core/resource';
import { useVariableStore } from './variableStore';
import { ProjectService, toFrontendGraph } from '@/services/project/projectService';
import { GraphService } from '@/services/graph/graphService';

vi.mock('@/services/project/projectService', () => ({
  ProjectService: {
    getProjectPath: vi.fn(),
    getDatabasesVariables: vi.fn(),
    getProjectIndex: vi.fn(),
    loadProjectGraph: vi.fn(),
  },
  toFrontendGraph: vi.fn(),
}));

vi.mock('@/services/graph/graphService', () => ({
  GraphService: {
    resolveGraphDynamicPins: vi.fn(),
  },
}));

vi.mock('@/features/application/graphDocument/functionSignatureSync', () => ({
  hydrateFunctionSignaturesFromProjectIndex: vi.fn(),
  syncFunctionSignatureFromGraph: vi.fn(),
}));

function makeEventGraphData(id: string, name: string): GraphData {
  return {
    id,
    name,
    type: 'event',
    canvas: { x: 5, y: 6, scale: 1 },
    nodes: [
      {
        id: 'node-a',
        graphId: id,
        nodeType: 'Control:Begin',
        category: ['Control'],
        title: 'Begin',
        position: { x: 0, y: 0 },
        inputs: [],
        outputs: ['pin-exec'],
        uiStyle: 'default',
      },
    ],
    pins: [
      {
        id: 'pin-exec',
        nodeId: 'node-a',
        name: 'Exec',
        type: 'exec',
        direction: 'output',
      },
    ],
    connections: [],
  };
}

describe('useProjectIOStore snapshot paths', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useGraphDataStore.setState({ graphEntities: {} });
    useDatabaseStore.getState().clear();
    useVariableStore.getState().clear();
    useResourceStore.getState().clear();
    useProjectIOStore.setState({ status: LoadStatus.Idle, error: null, currentPath: null });
  });

  it('loadProjectFromData merges database metadata and hydrates graphs', () => {
    useDatabaseStore.getState().setDatabases({
      'df-1': {
        id: 'df-1',
        name: 'Stored Name',
        rowCount: 99,
        columns: [{ name: 'amount', type: 'Float64' }],
      },
    });

    const project: ProjectData = {
      variables: {},
      databases: {
        'df-1': {
          id: 'df-1',
          engine: { csv: { path: '/data/sales.csv' } },
        },
      },
      graphs: {
        'evt-1': graphDataToDomainGraph(makeEventGraphData('evt-1', 'Main Event')),
      },
      metadata: { exportTime: '2026-07-08T00:00:00.000Z', appVersion: '1.0.0' },
    };

    useProjectIOStore.getState().loadProjectFromData(project, '/tmp/demo.yssbi');

    const storedDb = useDatabaseStore.getState().databases['df-1'];
    expect(storedDb.name).toBe('sales');
    expect(storedDb.rowCount).toBe(99);
    expect(storedDb.columns).toEqual([{ name: 'amount', type: 'Float64' }]);
    expect(storedDb.engine).toEqual({ csv: { path: '/data/sales.csv' } });
    expect(useGraphDataStore.getState().hasGraph('evt-1')).toBe(true);
    expect(useProjectIOStore.getState().currentPath).toBeTruthy();
  });

  it('exportSnapshot rebuilds graph structure from hydrated stores', () => {
    const project: ProjectData = {
      variables: {},
      databases: {},
      graphs: {
        'evt-1': graphDataToDomainGraph(makeEventGraphData('evt-1', 'Main Event')),
      },
      metadata: { exportTime: '2026-07-08T00:00:00.000Z', appVersion: '1.0.0' },
    };

    useProjectIOStore.getState().loadProjectFromData(project, null);
    const snapshot = useProjectIOStore.getState().exportSnapshot();

    expect(snapshot.graphs['evt-1']).toMatchObject({
      id: 'evt-1',
      name: 'Main Event',
      type: 'event',
      canvas: { x: 5, y: 6, scale: 1 },
    });
    expect(snapshot.graphs['evt-1'].nodes[0].outputs[0]).toMatchObject({ id: 'pin-exec', type: 'exec' });
    expect(snapshot.graphs['evt-1'].connections).toEqual({ connections: [] });
  });

  it('loadProject hydrates index and clears graph bodies', async () => {
    vi.mocked(ProjectService.getProjectPath).mockResolvedValue('/tmp/demo.yssbi');
    vi.mocked(ProjectService.getDatabasesVariables).mockResolvedValue({
      databases: { 'df-1': { id: 'df-1', name: 'Data' } },
      variables: {},
    });
    vi.mocked(ProjectService.getProjectIndex).mockResolvedValue({
      projectName: 'Demo',
      graphs: [{ id: 'evt-1', name: 'Main', type: 'event' }],
      variables: [],
      worksheets: [],
      exportTime: '2026-07-08T00:00:00.000Z',
      appVersion: '1.0.0',
    });

    const result = await useProjectIOStore.getState().loadProject();

    expect(result).not.toBeNull();
    expect(useProjectIOStore.getState().status).toBe(LoadStatus.Ready);
    expect(useGraphDataStore.getState().hasGraph('evt-1')).toBe(false);
    expect(useResourceStore.getState().graphOrder).toEqual(['evt-1']);
    expect(useDatabaseStore.getState().databases['df-1']?.name).toBe('Data');
  });

  it('loadGraph still calls backend when frontend cache exists but resource is not loaded', async () => {
    const graphData = makeEventGraphData('evt-1', 'Main Event');
    useGraphDataStore.getState().addGraphFromData('evt-1', graphData);
    useResourceStore.getState().setSnapshot({
      resources: [
        {
          id: 'evt-1',
          kind: 'event',
          name: 'Main Event',
          uri: 'yssbi://graph/event/evt-1',
          exists: true,
          loaded: false,
          hasDirtyDocument: false,
          hasStaleDocument: false,
          hasConflictDocument: false,
        },
      ],
      graphOrder: ['evt-1'],
    });

    vi.mocked(ProjectService.loadProjectGraph).mockResolvedValue({
      graph: {
        id: 'evt-1',
        name: 'Main Event',
        type: 'event',
        nodes: [],
        pins: [],
        connections: { connections: [] },
        canvas: { x: 0, y: 0, scale: 1 },
      },
      variables: {},
    });
    vi.mocked(toFrontendGraph).mockReturnValue(graphDataToDomainGraph(graphData));
    vi.mocked(GraphService.resolveGraphDynamicPins).mockResolvedValue(graphDataToDomainGraph(graphData));

    const loaded = await useProjectIOStore.getState().loadGraph('evt-1');

    expect(loaded).toBe(true);
    expect(ProjectService.loadProjectGraph).toHaveBeenCalledWith('evt-1');
  });
});
