import { beforeEach, describe, expect, it, vi } from 'vitest';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import { portAddressKey } from '@/features/domain/editorProjection';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';

import { useExecutionStore } from '@/features/core/execution';
import { executeCommand } from './commandExecutor';

const executeEditorMutation = vi.hoisted(() => vi.fn());

vi.mock('@/features/application/editorMutation/editorMutationCoordinator', () => ({
  executeEditorMutation,
}));
vi.mock('@/features/application/editorProjection/graphProjectionCoordinator', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/features/application/editorProjection/graphProjectionCoordinator')>()),
  currentProjectionLocale: () => 'en-US',
  hydrateGraphProjection: vi.fn(async () => true),
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

const graphPath = 'events/main.yssbi-event';

function installProjection() {
  const fixture = makeEditorProjectionFixture({ graphPath, sourceRevision: 3 });
  fixture.projection.nodes[0].ports[1] = {
    ...fixture.projection.nodes[0].ports[1],
    address: {
      kind: 'instance',
      nodeId: 'local-node',
      templateKey: 'inputs',
      instanceId: '00000000-0000-0000-0000-000000000011',
    },
    templateKey: 'inputs',
    instanceKind: 'userCreated',
    canRemove: true,
  };
  fixture.projection.connections[0] = {
    ...fixture.projection.connections[0],
    input: fixture.projection.nodes[0].ports[1].address,
  };
  useGraphDataStore.getState().replaceProjection(graphPath, fixture.projection, 1);
  return {
    outputKey: portAddressKey(fixture.projection.nodes[0].ports[0].address),
    inputKey: portAddressKey(fixture.projection.nodes[0].ports[1].address),
    outputAddress: fixture.projection.nodes[0].ports[0].address,
    inputAddress: fixture.projection.nodes[0].ports[1].address,
  };
}

describe('forward-only editor commands', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useGraphDataStore.setState({ graphEntities: {} });
  });

  it.each([
    {
      type: 'ConnectPins' as const,
      build: (fixture: ReturnType<typeof installProjection>) => ({
        pinA: fixture.inputKey,
        pinB: fixture.outputKey,
      }),
      mutation: (fixture: ReturnType<typeof installProjection>) => ({
        type: 'connect',
        payload: { output: fixture.outputAddress, input: fixture.inputAddress, order: null },
      }),
      multiple: false,
    },
    {
      type: 'DisconnectPin' as const,
      build: (fixture: ReturnType<typeof installProjection>) => ({ pinId: fixture.inputKey }),
      mutation: () => ({ type: 'disconnect', payload: { connectionId: 'local-connection' } }),
      multiple: true,
    },
    {
      type: 'SetPinValue' as const,
      build: (fixture: ReturnType<typeof installProjection>) => ({
        nodeId: 'local-node',
        pinId: fixture.inputKey,
        newValue: 42,
      }),
      mutation: (fixture: ReturnType<typeof installProjection>) => ({
        type: 'setLiteral',
        payload: { address: fixture.inputAddress, literal: 42 },
      }),
      multiple: false,
    },
    {
      type: 'DeleteNodes' as const,
      build: () => ({ nodeIds: ['local-node'] }),
      mutation: () => ({ type: 'deleteNode', payload: { nodeId: 'local-node' } }),
      multiple: true,
    },
    {
      type: 'AddRepeatablePin' as const,
      build: () => ({ nodeId: 'local-node', template: 'inputs' }),
      mutation: () => ({
        type: 'addPortInstance',
        payload: { nodeId: 'local-node', template: 'inputs', order: null },
      }),
      multiple: false,
    },
    {
      type: 'RemoveRepeatablePin' as const,
      build: (fixture: ReturnType<typeof installProjection>) => ({
        nodeId: 'local-node',
        pinId: fixture.inputKey,
      }),
      mutation: (fixture: ReturnType<typeof installProjection>) => ({
        type: 'removePortInstance',
        payload: { address: fixture.inputAddress },
      }),
      multiple: false,
    },
  ])('sends $type as a high-level intent without pre-response entity edits', async ({
    type,
    build,
    mutation,
  }) => {
    const fixture = installProjection();
    const before = useGraphDataStore.getState().graphEntities[graphPath];
    const pending = deferred<{ status: 'applied' }>();
    executeEditorMutation.mockReturnValueOnce(pending.promise);
    const randomId = vi.spyOn(crypto, 'randomUUID');

    const command = executeCommand(graphPath, type, build(fixture) as never);

    expect(executeEditorMutation).toHaveBeenCalledTimes(1);
    expect(executeEditorMutation).toHaveBeenCalledWith({
      graphPath,
      locale: 'en-US',
      mutation: mutation(fixture),
    });
    expect(useGraphDataStore.getState().graphEntities[graphPath]).toBe(before);
    expect(randomId).not.toHaveBeenCalled();

    pending.resolve({ status: 'applied' });
    await expect(command).resolves.toBe(true);
    randomId.mockRestore();
  });

  it('keeps MoveNodes forward-only and sends final positions unchanged', async () => {
    installProjection();
    executeEditorMutation.mockResolvedValueOnce({ status: 'applied' });

    await expect(executeCommand(graphPath, 'MoveNodes', {
      positions: [{ nodeId: 'local-node', position: { x: 11, y: 29 } }],
    })).resolves.toBe(true);

    expect(executeEditorMutation).toHaveBeenCalledWith({
      graphPath,
      locale: 'en-US',
      mutation: {
        type: 'moveNodes',
        payload: {
          positions: [{ nodeId: 'local-node', position: { x: 11, y: 29 } }],
        },
      },
    });
  });

  it('stops delete sequencing on conflict and does not mark the graph dirty', async () => {
    installProjection();
    const markGraphDirty = vi.spyOn(useExecutionStore.getState(), 'markGraphDirty');
    executeEditorMutation
      .mockResolvedValueOnce({ status: 'applied' })
      .mockResolvedValueOnce({ status: 'conflict' });

    await expect(executeCommand(graphPath, 'DeleteNodes', {
      nodeIds: ['node-1', 'node-2', 'node-3'],
    })).resolves.toBe(false);

    expect(executeEditorMutation).toHaveBeenCalledTimes(2);
    expect(executeEditorMutation.mock.calls[1]?.[0].mutation).toEqual({
      type: 'deleteNode',
      payload: { nodeId: 'node-2' },
    });
    expect(markGraphDirty).not.toHaveBeenCalled();
  });

  it('stops disconnect sequencing on stale without using the remaining connection snapshot', async () => {
    const fixture = installProjection();
    useGraphDataStore.setState((state) => {
      const bucket = state.graphEntities[graphPath];
      return {
        graphEntities: {
          ...state.graphEntities,
          [graphPath]: {
            ...bucket,
            connections: {
              ...bucket.connections,
              'connection-2': {
                ...bucket.connections['local-connection'],
                id: 'connection-2',
              },
            },
            pinConnections: {
              ...bucket.pinConnections,
              [fixture.inputKey]: ['local-connection', 'connection-2'],
            },
          },
        },
      };
    });
    const markGraphDirty = vi.spyOn(useExecutionStore.getState(), 'markGraphDirty');
    executeEditorMutation.mockResolvedValueOnce({ status: 'stale', result: {} });

    await expect(executeCommand(graphPath, 'DisconnectPin', {
      pinId: fixture.inputKey,
    })).resolves.toBe(false);

    expect(executeEditorMutation).toHaveBeenCalledTimes(1);
    expect(markGraphDirty).not.toHaveBeenCalled();
  });

  it('returns false for rejected commands without structural notification', async () => {
    installProjection();
    const markGraphDirty = vi.spyOn(useExecutionStore.getState(), 'markGraphDirty');
    executeEditorMutation.mockRejectedValueOnce(new Error('backend unavailable'));

    await expect(executeCommand(graphPath, 'ConnectPins', {
      pinA: installProjection().inputKey,
      pinB: installProjection().outputKey,
    })).resolves.toBe(false);

    expect(markGraphDirty).not.toHaveBeenCalled();
  });
});
