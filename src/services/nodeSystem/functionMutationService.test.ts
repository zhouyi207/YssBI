import { invoke } from '@tauri-apps/api/core';
import { describe, expect, it, vi } from 'vitest';
import type {
  FunctionDocumentPatchDto,
  MutationRequestDto,
  ResourceMutationResultDto,
} from '@/shared/types/dto/editorMutation';
import { FunctionMutationService } from './functionMutationService';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

describe('FunctionMutationService', () => {
  it('uses the revisioned function-signature command wire', async () => {
    const request: MutationRequestDto<FunctionDocumentPatchDto> = {
      resource: { kind: 'function', key: 'functions/Compute.yssbi-function' },
      baseRevision: 4,
      operationId: '00000000-0000-0000-0000-000000000504',
      payload: {
        before: { parameters: [], return_type: null },
        after: { parameters: [], return_type: 'Float64' },
      },
    };
    const result: ResourceMutationResultDto = {
      operationId: request.operationId,
      projectInstanceId: '00000000-0000-0000-0000-000000000601',
      publicationRevision: 6,
      moves: [],
      deltas: [],
      projectionReplacements: [],
      projectionStatus: { status: 'complete', expectedGraphPaths: [] },
      history: { canUndo: true, canRedo: false },
    };
    vi.mocked(invoke).mockResolvedValue(result);

    await expect(FunctionMutationService.updateSignature(
      '00000000-0000-0000-0000-000000000601',
      'functions/Compute.yssbi-function',
      'zh-CN',
      request,
    )).resolves.toBe(result);

    expect(invoke).toHaveBeenCalledWith('update_function_signature', {
      projectInstanceId: '00000000-0000-0000-0000-000000000601',
      functionPath: 'functions/Compute.yssbi-function',
      locale: 'zh-CN',
      request,
    });
  });

  it('preserves project identity when the command rejects', async () => {
    const request: MutationRequestDto<FunctionDocumentPatchDto> = {
      resource: { kind: 'function', key: 'functions/Compute.yssbi-function' },
      baseRevision: 4,
      operationId: '00000000-0000-0000-0000-000000000505',
      payload: {
        before: { parameters: [], return_type: null },
        after: { parameters: [], return_type: 'Float64' },
      },
    };
    const rejection = { code: 'stale_project_lifecycle', message: 'project was replaced' };
    vi.mocked(invoke).mockRejectedValue(rejection);

    await expect(FunctionMutationService.updateSignature(
      '00000000-0000-0000-0000-000000000601',
      'functions/Compute.yssbi-function',
      'en-US',
      request,
    )).rejects.toBe(rejection);

    expect(invoke).toHaveBeenCalledWith('update_function_signature', {
      projectInstanceId: '00000000-0000-0000-0000-000000000601',
      functionPath: 'functions/Compute.yssbi-function',
      locale: 'en-US',
      request,
    });
  });
});
