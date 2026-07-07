import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useGraphDataStore, useGraphMetaStore } from '@/features/core/dataStore';
import { GraphService } from '@/services/graph/graphService';
import { updateFunctionSignature } from './graphDocumentActions';

describe('graphDocumentActions', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    useGraphMetaStore.setState({ graphs: {}, graphOrder: [] });
    useGraphDataStore.setState({ graphEntities: {} });
  });

  it('updates function signature through the narrow service API and stores the returned graph', async () => {
    const inputs = [{ id: 'input-1', name: 'Value', type: 'int' }];
    const outputs = [{ id: 'output-1', name: 'Result', type: 'float', containerType: 'array' }];
    const serviceSpy = vi.spyOn(GraphService, 'updateFunctionSignature').mockResolvedValue({
      graph: {
        id: 'function-1',
        name: 'Compute',
        type: 'function',
        functionInputs: inputs,
        functionOutputs: outputs,
        nodes: [],
        pins: [],
        connections: { connections: [] },
        canvas: { x: 0, y: 0, scale: 1 },
      },
      callerGraphs: [],
      sideEffectWarning: false,
    });

    await updateFunctionSignature('function-1', { inputs });

    expect(serviceSpy).toHaveBeenCalledWith('function-1', { inputs });
    expect(useGraphMetaStore.getState().graphs['function-1']).toEqual(
      expect.objectContaining({
        functionInputs: inputs,
        functionOutputs: outputs,
      }),
    );
    expect(useGraphDataStore.getState().getGraphNodeIds('function-1')).toEqual([]);
  });
});
