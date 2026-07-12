import { beforeEach, describe, expect, it } from 'vitest';
import { createDataSignaturePin } from '@/shared/types/domain/functionSignaturePin';
import { useGraphMetaStore } from '@/features/core/dataStore/graphMetaStore';
import {
  syncFunctionSignatureFromGraph,
  hydrateFunctionSignaturesFromProjectIndex,
} from './functionSignatureSync';

describe('functionSignatureSync', () => {
  beforeEach(() => {
    useGraphMetaStore.setState({ graphs: {} });
  });

  it('writes function signature fields into graph meta store', () => {
    syncFunctionSignatureFromGraph({
      path: 'function-1',
      name: 'Compute',
      type: 'function',
      functionInputs: [createDataSignaturePin('input-1', 'Value', { kind: 'Int64' })],
      functionOutputs: [createDataSignaturePin('output-1', 'Result', { kind: 'Float64' })],
    });

    expect(useGraphMetaStore.getState().graphs['function-1']).toEqual(
      expect.objectContaining({
        functionInputs: [createDataSignaturePin('input-1', 'Value', { kind: 'Int64' })],
        functionOutputs: [createDataSignaturePin('output-1', 'Result', { kind: 'Float64' })],
      }),
    );
  });

  it('hydrates signatures from project index rows', () => {
    hydrateFunctionSignaturesFromProjectIndex([
      {
        path: 'functions/Add.yssbi-function',
        name: 'Add',
        type: 'function',
        functionInputs: [createDataSignaturePin('a', 'A', { kind: 'Int64' })],
        functionOutputs: [createDataSignaturePin('r', 'R', { kind: 'Int64' })],
      },
      { path: 'events/Main.yssbi-event', name: 'Main', type: 'event' },
    ]);

    expect(useGraphMetaStore.getState().graphs['functions/Add.yssbi-function']).toEqual(
      expect.objectContaining({
        functionInputs: [createDataSignaturePin('a', 'A', { kind: 'Int64' })],
        functionOutputs: [createDataSignaturePin('r', 'R', { kind: 'Int64' })],
      }),
    );
    expect(useGraphMetaStore.getState().graphs['events/Main.yssbi-event']).toBeUndefined();
  });
});
