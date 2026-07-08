import { beforeEach, describe, expect, it } from 'vitest';
import { useGraphMetaStore } from '@/features/core/dataStore/graphMetaStore';
import {
  syncFunctionSignatureFromGraph,
  hydrateFunctionSignaturesFromProjectIndex,
} from './functionSignatureSync';

describe('functionSignatureSync', () => {
  beforeEach(() => {
    useGraphMetaStore.setState({ graphs: {}, graphOrder: [] });
  });

  it('writes function signature fields into graph meta store', () => {
    syncFunctionSignatureFromGraph({
      path: 'function-1',
      name: 'Compute',
      type: 'function',
      functionInputs: [{ id: 'input-1', name: 'Value', type: 'int' }],
      functionOutputs: [{ id: 'output-1', name: 'Result', type: 'float' }],
    });

    expect(useGraphMetaStore.getState().graphs['function-1']).toEqual(
      expect.objectContaining({
        functionInputs: [{ id: 'input-1', name: 'Value', type: 'int' }],
        functionOutputs: [{ id: 'output-1', name: 'Result', type: 'float' }],
      }),
    );
  });

  it('hydrates signatures from project index rows', () => {
    hydrateFunctionSignaturesFromProjectIndex([
      {
        path: 'functions/Add.yssbi-function',
        name: 'Add',
        type: 'function',
        functionInputs: [{ id: 'a', name: 'A', type: 'int' }],
        functionOutputs: [{ id: 'r', name: 'R', type: 'int' }],
      },
      { path: 'events/Main.yssbi-event', name: 'Main', type: 'event' },
    ]);

    expect(useGraphMetaStore.getState().graphs['functions/Add.yssbi-function']).toEqual(
      expect.objectContaining({
        functionInputs: [{ id: 'a', name: 'A', type: 'int' }],
        functionOutputs: [{ id: 'r', name: 'R', type: 'int' }],
      }),
    );
    expect(useGraphMetaStore.getState().graphs['events/Main.yssbi-event']).toBeUndefined();
  });
});
