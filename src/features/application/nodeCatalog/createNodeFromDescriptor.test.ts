import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { NodeCreationDescriptor } from '@/features/domain/nodeCatalog/creationDescriptor';
import type { PortAddressDto } from '@/shared/types/dto/editorProjection';
import { executeEditorMutation } from '@/features/application/editorMutation/editorMutationCoordinator';
import { createNodeFromDescriptor } from './createNodeFromDescriptor';

vi.mock('@/features/application/editorMutation/editorMutationCoordinator', () => ({
  executeEditorMutation: vi.fn(),
}));

describe('createNodeFromDescriptor', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('sends the exact static descriptor and position through the mutation coordinator', async () => {
    const outcome = { status: 'conflict' as const };
    vi.mocked(executeEditorMutation).mockResolvedValue(outcome);
    const descriptor: NodeCreationDescriptor = {
      kind: 'static',
      nodeTypeId: 'math.add',
    };

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
          descriptor,
          position: { x: 12, y: 34 },
          userLabel: null,
          connectFrom: null,
        },
      },
    });
  });

  it('sends the exact parameterized-static descriptor unchanged', async () => {
    const outcome = { status: 'conflict' as const };
    vi.mocked(executeEditorMutation).mockResolvedValue(outcome);
    const descriptor: NodeCreationDescriptor = {
      kind: 'parameterizedStatic',
      nodeTypeId: 'yssbi.dataframe.project',
      requiredParameters: ['columns'],
    };

    await expect(createNodeFromDescriptor({
      graphPath: 'events/Main.yssbi-event',
      locale: 'en-US',
      descriptor,
      position: { x: 3, y: 7 },
    })).resolves.toBe(outcome);

    expect(executeEditorMutation).toHaveBeenCalledOnce();
    expect(executeEditorMutation).toHaveBeenCalledWith(expect.objectContaining({
      mutation: {
        type: 'createNode',
        payload: { descriptor, position: { x: 3, y: 7 }, userLabel: null, connectFrom: null },
      },
    }));
  });

  it('sends the exact resource-bound descriptor unchanged', async () => {
    const outcome = { status: 'conflict' as const };
    vi.mocked(executeEditorMutation).mockResolvedValue(outcome);
    const descriptor: NodeCreationDescriptor = {
      kind: 'resourceBound',
      nodeTypeId: 'functions.call',
      resourcePath: 'functions/Helper.yssbi-function',
      resourceRevision: 4,
      createArgs: { kind: 'function' },
    };

    await expect(createNodeFromDescriptor({
      graphPath: 'events/Main.yssbi-event',
      locale: 'zh-CN',
      descriptor,
      position: { x: 5, y: 8 },
    })).resolves.toBe(outcome);

    expect(executeEditorMutation).toHaveBeenCalledWith(expect.objectContaining({
      mutation: {
        type: 'createNode',
        payload: { descriptor, position: { x: 5, y: 8 }, userLabel: null, connectFrom: null },
      },
    }));
  });

  it('forwards an exact source port for atomic create and connect', async () => {
    const outcome = { status: 'conflict' as const };
    vi.mocked(executeEditorMutation).mockResolvedValue(outcome);
    const descriptor: NodeCreationDescriptor = { kind: 'static', nodeTypeId: 'math.add' };
    const connectFrom: PortAddressDto = {
      kind: 'declared',
      nodeId: '00000000-0000-0000-0000-000000000101',
      portKey: 'value',
    };

    await createNodeFromDescriptor({
      graphPath: 'events/Main.yssbi-event',
      locale: 'en-US',
      descriptor,
      position: { x: 5, y: 8 },
      connectFrom,
    });

    expect(executeEditorMutation).toHaveBeenCalledWith(expect.objectContaining({
      mutation: {
        type: 'createNode',
        payload: { descriptor, position: { x: 5, y: 8 }, userLabel: null, connectFrom },
      },
    }));
  });

  it('rejects descriptors with compatibility fields before mutation execution', async () => {
    const descriptor = {
      kind: 'static',
      nodeTypeId: 'math.add',
      parameters: {},
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
