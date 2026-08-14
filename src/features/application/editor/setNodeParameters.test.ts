import { beforeEach, describe, expect, it, vi } from 'vitest';
import { executeEditorMutation } from '@/features/application/editorMutation/editorMutationCoordinator';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { setNodeParameters } from './setNodeParameters';

vi.mock('@/features/application/editorMutation/editorMutationCoordinator', () => ({
  executeEditorMutation: vi.fn(),
}));

describe('setNodeParameters', () => {
  beforeEach(() => vi.clearAllMocks());

  it('forwards one exact atomic parameter mutation through the revision coordinator', async () => {
    const outcome = { status: 'applied' as const, result: {} as never };
    vi.mocked(executeEditorMutation).mockResolvedValue(outcome);
    const parameters = {
      predicate: {
        column: 'count',
        operator: 'greaterThan',
        value: { type: 'integer', value: '9007199254740993' },
      },
    };

    await expect(setNodeParameters({
      graphPath: 'events/Main.yssbi-event',
      nodeId: 'node-1',
      locale: 'en-US',
      parameters,
    })).resolves.toBe(outcome);

    expect(executeEditorMutation).toHaveBeenCalledOnce();
    expect(executeEditorMutation).toHaveBeenCalledWith({
      graphPath: 'events/Main.yssbi-event',
      locale: 'en-US',
      mutation: {
        type: 'setParameters',
        payload: { nodeId: 'node-1', parameters },
      },
    });
  });

  it('removes a null override while preserving the complete atomic parameter map', async () => {
    const outcome = { status: 'applied' as const, result: {} as never };
    vi.mocked(executeEditorMutation).mockResolvedValue(outcome);
    vi.spyOn(useGraphDataStore, 'getState').mockReturnValue({
      getGraphNode: () => ({
        parameterEditors: [
          { key: 'constant', value: true },
          { key: 'convergence_tolerance', value: 1e-7 },
          { key: 'missing_value_policy', value: 'Reject' },
        ],
      }),
    } as never);

    await setNodeParameters({
      graphPath: 'events/Main.yssbi-event',
      nodeId: 'node-1',
      locale: 'en-US',
      parameters: { convergence_tolerance: null },
    });

    expect(executeEditorMutation).toHaveBeenCalledWith(expect.objectContaining({
      mutation: {
        type: 'setParameters',
        payload: {
          nodeId: 'node-1',
          parameters: { constant: true, missing_value_policy: 'Reject' },
        },
      },
    }));
  });
});
