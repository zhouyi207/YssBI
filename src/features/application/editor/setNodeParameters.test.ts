import { beforeEach, describe, expect, it, vi } from 'vitest';
import { executeEditorMutation } from '@/features/application/editorMutation/editorMutationCoordinator';
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
});
