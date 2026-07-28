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



  it('never regresses signature metadata from an older or inconsistent equal index row', () => {
    const path = 'functions/Monotonic.yssbi-function';
    const currentSignature = {
      parameters: [{ id: 'current', name: 'Current', type_name: 'Float64' }],
      return_type: 'Float64',
    };
    hydrateFunctionSignaturesFromProjectIndex([{
      path,
      name: 'Monotonic',
      type: 'function',
      functionRevision: 9,
      functionSignature: currentSignature,
    }]);

    hydrateFunctionSignaturesFromProjectIndex([{
      path,
      name: 'Monotonic',
      type: 'function',
      functionRevision: 8,
      functionSignature: { parameters: [], return_type: null },
    }, {
      path,
      name: 'Monotonic',
      type: 'function',
      functionRevision: 9,
      functionSignature: { parameters: [], return_type: 'Int64' },
    }]);

    expect(useGraphMetaStore.getState().graphs[path]).toMatchObject({
      functionRevision: 9,
      functionSignature: currentSignature,
      functionInputs: [createDataSignaturePin('current', 'Current', { kind: 'Float64' })],
    });
  });

  it('hydrates signatures from project index rows', () => {
    hydrateFunctionSignaturesFromProjectIndex([
      {
        path: 'functions/Add.yssbi-function',
        name: 'Add',
        type: 'function',
        functionRevision: 7,
        functionSignature: {
          parameters: [{ id: 'a', name: 'A', type_name: 'Int64' }],
          return_type: 'Int64',
        },
      },
      { path: 'events/Main.yssbi-event', name: 'Main', type: 'event' },
    ]);

    expect(useGraphMetaStore.getState().graphs['functions/Add.yssbi-function']).toEqual(
      expect.objectContaining({
        functionRevision: 7,
        functionSignature: {
          parameters: [{ id: 'a', name: 'A', type_name: 'Int64' }],
          return_type: 'Int64',
        },
        functionInputs: [createDataSignaturePin('a', 'A', { kind: 'Int64' })],
        functionOutputs: [createDataSignaturePin('return', 'Result', { kind: 'Int64' })],
      }),
    );
    expect(useGraphMetaStore.getState().graphs['events/Main.yssbi-event']).toBeUndefined();
  });
});
