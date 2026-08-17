import { invokeCommand } from '@/services/ipc';
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
    return invokeCommand<ResourceMutationResultDto>('update_function_signature', {
      projectInstanceId,
      functionPath,
      locale,
      request,
    });
  }
}
