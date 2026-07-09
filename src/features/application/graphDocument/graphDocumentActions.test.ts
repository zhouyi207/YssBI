import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createDataSignaturePin } from '@/shared/types/domain/functionSignaturePin';
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
    const inputs = [createDataSignaturePin('input-1', 'Value', { kind: 'Int64' })];
    const outputs = [
      createDataSignaturePin('output-1', 'Result', {
        kind: 'Array',
        inner: { kind: 'Float64' },
      }),
    ];
    const serviceSpy = vi.spyOn(GraphService, 'updateFunctionSignature').mockResolvedValue({
      graph: {
        path: 'function-1',
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
