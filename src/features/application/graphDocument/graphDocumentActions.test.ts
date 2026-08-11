import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createDataSignaturePin } from '@/shared/types/domain/functionSignaturePin';
import { useGraphDataStore, useGraphMetaStore } from '@/features/core/dataStore';
import * as graphDocumentActions from './graphDocumentActions';

const commitFunctionSignature = vi.hoisted(() => vi.fn());

vi.mock('@/features/application/editorMutation/functionSignatureCoordinator', () => ({
  commitFunctionSignature,
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

    await graphDocumentActions.updateFunctionSignature('functions/Compute.yssbi-function', { inputs });

    expect(commitFunctionSignature).toHaveBeenCalledWith(
      'functions/Compute.yssbi-function',
      { inputs },
    );
    expect(useGraphMetaStore.getState().graphs).toEqual({});
    expect(useGraphDataStore.getState().graphEntities).toEqual({});
  });
});
