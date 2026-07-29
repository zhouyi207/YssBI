import { invoke } from '@tauri-apps/api/core';
import type {
  FunctionDocumentPatchDto,
  MutationRequestDto,
  ResourceMutationResultDto,
} from '@/shared/types/dto/editorMutation';

export class FunctionMutationService {
  static updateSignature(
    projectInstanceId: string,
    functionPath: string,
    locale: string,
    request: MutationRequestDto<FunctionDocumentPatchDto>,
  ): Promise<ResourceMutationResultDto> {
    return invoke<ResourceMutationResultDto>('update_function_signature', {
      projectInstanceId,
      functionPath,
      locale,
      request,
    });
  }
}
