import { invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import type {
  EditorGraphMutationDto,
  MutationRequestDto,
} from '@/shared/types/dto/editorMutation';
import { GraphMutationService } from './graphMutationService';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const projectInstanceId = '00000000-0000-0000-0000-000000000601';
const graphPath = 'functions/Main.yssbi-function';
const operationId = '00000000-0000-0000-0000-000000000602';

const request: MutationRequestDto<EditorGraphMutationDto> = {
  resource: { kind: 'graph', key: graphPath },
  baseRevision: 1,
  operationId,
  payload: { type: 'deleteNode', payload: { nodeId: 'local-node' } },
};

function graphMutationWireResult(): unknown {
  return {
    projectInstanceId,
    delta: {
      graphPath,
      fromRevision: 1,
      toRevision: 2,
      causedBy: operationId,
      payload: { operations: [] },
    },
    projectionReplacement: {
      graphPath,
      projection: makeEditorProjectionFixture({
        graphPath,
        sourceRevision: 2,
        nodeId: '00000000-0000-0000-0000-000000000603',
        title: 'Committed',
      }).projection,
      functionEditorProjection: {
        functionRevision: 2,
        inputs: [],
        outputs: [],
      },
    },
    history: { canUndo: true, canRedo: false },
  };
}

describe('GraphMutationService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('sends the captured project identity and parses the mutation result boundary', async () => {
    const response = graphMutationWireResult();
    vi.mocked(invoke).mockResolvedValue(response);

    await expect(GraphMutationService.mutateGraph(
      projectInstanceId,
      graphPath,
      'en-US',
      request,
    )).resolves.toEqual(response);

    expect(invoke).toHaveBeenCalledWith('mutate_graph_document', {
      projectInstanceId,
      graphPath,
      locale: 'en-US',
      request,
    });
  });

  it('rejects a mutation response without required lifecycle identity', async () => {
    const { projectInstanceId: _omitted, ...malformed } = graphMutationWireResult() as Record<string, unknown>;
    vi.mocked(invoke).mockResolvedValue(malformed);

    await expect(GraphMutationService.mutateGraph(
      projectInstanceId,
      graphPath,
      'en-US',
      request,
    )).rejects.toThrow(/projectInstanceId/);
  });
});
