import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createDataSignaturePin } from '@/shared/types/domain/functionSignaturePin';
import { useGraphDataStore, useGraphMetaStore } from '@/features/core/dataStore';
import { GraphService } from '@/services/graph/graphService';
import { invalidateGraphProjection, invalidateGraphProjections } from '@/features/application/editorProjection/graphProjectionCoordinator';
import { updateCallFunctionTarget, updateFunctionSignature } from './graphDocumentActions';

const commitFunctionSignature = vi.hoisted(() => vi.fn());

vi.mock('@/features/application/editorMutation/functionSignatureCoordinator', () => ({
  commitFunctionSignature,
}));
vi.mock('@/features/application/editorProjection/graphProjectionCoordinator', () => ({
  currentProjectionLocale: () => 'en-US',
  hydrateGraphProjection: vi.fn(async () => true),
  invalidateGraphProjection: vi.fn(async () => true),
  invalidateGraphProjections: vi.fn(async () => undefined),
}));

describe('graphDocumentActions', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useGraphMetaStore.setState({ graphs: {} });
    useGraphDataStore.setState({ graphEntities: {} });
  });

  it('delegates signature edits to the revisioned authoritative coordinator', async () => {
    const inputs = [createDataSignaturePin('input-1', 'Value', { kind: 'Int64' })];
    commitFunctionSignature.mockResolvedValueOnce({ status: 'applied' });

    await updateFunctionSignature('functions/Compute.yssbi-function', { inputs });

    expect(commitFunctionSignature).toHaveBeenCalledWith(
      'functions/Compute.yssbi-function',
      { inputs },
    );
    expect('updateFunctionSignature' in GraphService).toBe(false);
    expect(useGraphMetaStore.getState().graphs).toEqual({});
    expect(useGraphDataStore.getState().graphEntities).toEqual({});
    expect(invalidateGraphProjections).not.toHaveBeenCalled();
  });

  it('refreshes a Call Function graph instead of patching legacy target fields', async () => {
    vi.spyOn(GraphService, 'updateCallFunctionTarget').mockResolvedValue(undefined);

    await updateCallFunctionTarget('events/Main.yssbi-event', 'call-1', 'functions/Next');

    expect(invalidateGraphProjection).toHaveBeenCalledWith('events/Main.yssbi-event');
    expect(useGraphDataStore.getState().hasGraph('events/Main.yssbi-event')).toBe(false);
  });
});
