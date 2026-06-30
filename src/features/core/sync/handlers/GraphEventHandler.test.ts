import { beforeEach, describe, expect, it } from 'vitest';
import { useGraphMetaStore } from '@/features/core/dataStore';
import { useResourceStore } from '@/features/core/resource';
import { FunctionCreatedHandler, FunctionUpdatedHandler } from './GraphEventHandler';

describe('Graph event handlers', () => {
  beforeEach(() => {
    useGraphMetaStore.setState({ graphs: {}, graphOrder: [], graphFolders: [] });
    useResourceStore.getState().clear();
  });

  it('syncs function signature metadata from FunctionCreated events', () => {
    new FunctionCreatedHandler().handle({
      id: 'function-1',
      data: {
        id: 'function-1',
        name: 'Compute',
        type: 'function',
        functionInputs: [{ id: 'input-1', name: 'Value', type: 'int' }],
        functionOutputs: [{ id: 'output-1', name: 'Result', type: 'float' }],
        nodes: [],
        pins: [],
        connections: { connections: [] },
        canvas: { x: 0, y: 0, scale: 1 },
      },
    });

    expect(useGraphMetaStore.getState().graphs['function-1']).toEqual(
      expect.objectContaining({
        functionInputs: [{ id: 'input-1', name: 'Value', type: 'int' }],
        functionOutputs: [{ id: 'output-1', name: 'Result', type: 'float' }],
      }),
    );
  });

  it('syncs function signature metadata from FunctionUpdated events', () => {
    useResourceStore.getState().upsertResource({
      id: 'function-1',
      kind: 'function',
      name: 'Compute',
      uri: 'yssbi://graph/function/function-1',
      exists: true,
      loaded: true,
      hasDirtyDocument: false,
      hasStaleDocument: false,
      hasConflictDocument: false,
    });

    new FunctionUpdatedHandler().handle({
      id: 'function-1',
      data: {
        id: 'function-1',
        name: 'Compute',
        type: 'function',
        functionInputs: [{ id: 'input-1', name: 'Value', type: 'int' }],
        functionOutputs: [{ id: 'output-1', name: 'Result', type: 'float' }],
      },
    });

    expect(useGraphMetaStore.getState().graphs['function-1']).toEqual(
      expect.objectContaining({
        functionInputs: [{ id: 'input-1', name: 'Value', type: 'int' }],
        functionOutputs: [{ id: 'output-1', name: 'Result', type: 'float' }],
      }),
    );
  });

  it('does not create function metadata from partial FunctionUpdated events without resource metadata', () => {
    new FunctionUpdatedHandler().handle({
      id: 'function-1',
      data: {
        functionInputs: [{ id: 'input-1', name: 'Value', type: 'int' }],
      },
    });

    expect(useGraphMetaStore.getState().graphs['function-1']).toBeUndefined();
  });
});
