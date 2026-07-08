import { beforeEach, describe, expect, it } from 'vitest';
import { useGraphMetaStore } from '@/features/core/dataStore';
import { buildGraphResourceMeta, useResourceStore } from '@/features/core/resource';
import { FunctionCreatedHandler, FunctionUpdatedHandler } from './GraphEventHandler';

describe('Graph event handlers', () => {
  beforeEach(() => {
    useGraphMetaStore.setState({ graphs: {}, graphOrder: [] });
    useResourceStore.getState().clear();
  });

  it('syncs function signature metadata from FunctionCreated events', () => {
    new FunctionCreatedHandler().handle({
      path: 'functions/Compute.yssbi-function',
      data: {
        path: 'functions/Compute.yssbi-function',
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

    expect(useGraphMetaStore.getState().graphs['functions/Compute.yssbi-function']).toEqual(
      expect.objectContaining({
        functionInputs: [{ id: 'input-1', name: 'Value', type: 'int' }],
        functionOutputs: [{ id: 'output-1', name: 'Result', type: 'float' }],
      }),
    );
  });

  it('syncs function signature metadata from FunctionUpdated events', () => {
    useResourceStore.getState().upsertResource(
      buildGraphResourceMeta('function', 'functions/Compute.yssbi-function', 'Compute', { loaded: true }),
    );

    new FunctionUpdatedHandler().handle({
      path: 'functions/Compute.yssbi-function',
      data: {
        path: 'functions/Compute.yssbi-function',
        name: 'Compute',
        type: 'function',
        functionInputs: [{ id: 'input-1', name: 'Value', type: 'int' }],
        functionOutputs: [{ id: 'output-1', name: 'Result', type: 'float' }],
      },
    });

    expect(useGraphMetaStore.getState().graphs['functions/Compute.yssbi-function']).toEqual(
      expect.objectContaining({
        functionInputs: [{ id: 'input-1', name: 'Value', type: 'int' }],
        functionOutputs: [{ id: 'output-1', name: 'Result', type: 'float' }],
      }),
    );
  });

  it('does not create function metadata from partial FunctionUpdated events without resource metadata', () => {
    new FunctionUpdatedHandler().handle({
      path: 'functions/Compute.yssbi-function',
      data: {
        functionInputs: [{ id: 'input-1', name: 'Value', type: 'int' }],
      },
    });

    expect(useGraphMetaStore.getState().graphs['functions/Compute.yssbi-function']).toBeUndefined();
  });
});
