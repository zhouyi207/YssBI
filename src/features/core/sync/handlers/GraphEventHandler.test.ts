import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createDataSignaturePin } from '@/shared/types/domain/functionSignaturePin';
import { useGraphMetaStore } from '@/features/core/dataStore';
import { buildGraphResourceMeta, useResourceStore } from '@/features/core/resource';
import { EventUpdatedHandler, FunctionUpdatedHandler } from './GraphEventHandler';
import { invalidateGraphProjection } from '@/features/application/editorProjection/graphProjectionCoordinator';

vi.mock('@/features/application/editorProjection/graphProjectionCoordinator', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/features/application/editorProjection/graphProjectionCoordinator')>()),
  invalidateGraphProjection: vi.fn(async () => true),
}));


describe('Graph event handlers', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useGraphMetaStore.setState({ graphs: {} });
    useResourceStore.getState().clear();
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
        functionInputs: [createDataSignaturePin('input-1', 'Value', { kind: 'Int64' })],
        functionOutputs: [createDataSignaturePin('output-1', 'Result', { kind: 'Float64' })],
      },
    });

    expect(useGraphMetaStore.getState().graphs['functions/Compute.yssbi-function']).toEqual(
      expect.objectContaining({
        functionInputs: [createDataSignaturePin('input-1', 'Value', { kind: 'Int64' })],
        functionOutputs: [createDataSignaturePin('output-1', 'Result', { kind: 'Float64' })],
      }),
    );
    expect(invalidateGraphProjection).toHaveBeenCalledWith('functions/Compute.yssbi-function');
  });

  it('invalidates an updated event projection', () => {
    new EventUpdatedHandler().handle({
      path: 'events/Main.yssbi-event',
      data: { name: 'Main' },
    });

    expect(invalidateGraphProjection).toHaveBeenCalledWith('events/Main.yssbi-event');
  });

  it('does not create function metadata from partial FunctionUpdated events without resource metadata', () => {
    new FunctionUpdatedHandler().handle({
      path: 'functions/Compute.yssbi-function',
      data: {
        functionInputs: [createDataSignaturePin('input-1', 'Value', { kind: 'Int64' })],
      },
    });

    expect(useGraphMetaStore.getState().graphs['functions/Compute.yssbi-function']).toBeUndefined();
  });

});
