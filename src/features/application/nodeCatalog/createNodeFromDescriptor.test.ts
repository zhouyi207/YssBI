import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { NodeCreationDescriptor } from '@/features/domain/nodeCatalog/creationDescriptor';
import { executeEditorMutation } from '@/features/application/editorMutation/editorMutationCoordinator';
import { createNodeFromDescriptor } from './createNodeFromDescriptor';

vi.mock('@/features/application/editorMutation/editorMutationCoordinator', () => ({
  executeEditorMutation: vi.fn(),
}));

describe('createNodeFromDescriptor', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('sends only the static descriptor node type and position through the mutation coordinator', async () => {
    const outcome = { status: 'conflict' as const };
    vi.mocked(executeEditorMutation).mockResolvedValue(outcome);
    const descriptor = {
      kind: 'static',
      nodeTypeId: 'math.add',
      nodeId: 'frontend-node-id',
      ports: [{ id: 'frontend-port-id' }],
      inferredTypes: { value: 'Float64' },
      dynamicInterfaces: [{ template: 'inputs' }],
      compatibilityData: { accepts: ['Float64'] },
      parameters: { arbitrary: true },
    } as unknown as NodeCreationDescriptor;

    await expect(createNodeFromDescriptor({
      graphPath: 'functions/Main.yssbi-function',
      locale: 'en-US',
      descriptor,
      position: { x: 12, y: 34 },
    })).resolves.toBe(outcome);

    expect(executeEditorMutation).toHaveBeenCalledTimes(1);
    expect(executeEditorMutation).toHaveBeenCalledWith({
      graphPath: 'functions/Main.yssbi-function',
      locale: 'en-US',
      mutation: {
        type: 'createNode',
        payload: {
          nodeTypeId: 'math.add',
          position: { x: 12, y: 34 },
          parameters: {},
          userLabel: null,
        },
      },
    });
  });

  it('rejects resource-bound descriptors before mutation execution', async () => {
    const descriptor = {
      kind: 'resourceBound',
      nodeTypeId: 'functions.call',
      resourcePath: 'functions/Helper.yssbi-function',
    } as unknown as NodeCreationDescriptor;

    await expect(createNodeFromDescriptor({
      graphPath: 'events/Main.yssbi-event',
      locale: 'zh-CN',
      descriptor,
      position: { x: 5, y: 8 },
    })).rejects.toThrow('Unsupported node creation descriptor');

    expect(executeEditorMutation).not.toHaveBeenCalled();
  });
});
