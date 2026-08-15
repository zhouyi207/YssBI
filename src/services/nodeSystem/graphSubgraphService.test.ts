import { invoke } from '@tauri-apps/api/core';
import { describe, expect, it, vi } from 'vitest';
import { GraphSubgraphService } from './graphSubgraphService';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const completeSnapshot = {
  schemaVersion: 1,
  nodes: [{
    localId: 'node/0',
    creation: { kind: 'static', nodeTypeId: 'yssbi.constant.int64' },
    parameters: {},
    userLabel: null,
    relativePosition: { x: 0, y: 0 },
  }],
  portBindings: [],
  inputStates: [],
  connections: [],
};

describe('GraphSubgraphService', () => {
  it('exports through the identity-scoped command and parses the response', async () => {
    vi.mocked(invoke).mockResolvedValue(completeSnapshot);

    await expect(GraphSubgraphService.exportSubgraph(
      'project-a',
      'events/main.yssbi-event',
      ['node-a', 'node-b'],
    )).resolves.toEqual(completeSnapshot);

    expect(invoke).toHaveBeenCalledWith('export_graph_subgraph', {
      projectInstanceId: 'project-a',
      graphPath: 'events/main.yssbi-event',
      nodeIds: ['node-a', 'node-b'],
    });
  });

  it('rejects malformed command responses at the service boundary', async () => {
    vi.mocked(invoke).mockResolvedValue({ ...completeSnapshot, schemaVersion: 2 });

    await expect(GraphSubgraphService.exportSubgraph(
      'project-a',
      'events/main.yssbi-event',
      ['node-a'],
    )).rejects.toThrow('Invalid clipboard subgraph response');
  });
});
