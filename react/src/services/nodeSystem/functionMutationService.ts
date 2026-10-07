import { invokeCommand } from "@/services/ipc";
import { parseResourceMutationResultDto } from "@/shared/types/dto/resourceMutationResultWireParser";
import type {
  FunctionDocumentPatchDto,
  MutationRequestDto,
  ResourceMutationResultDto,
} from "@/shared/types/dto/editorMutation";

export class FunctionMutationService {
  static async updateSignature(
    projectInstanceId: string,
    functionPath: string,
    request: MutationRequestDto<FunctionDocumentPatchDto>,
  ): Promise<ResourceMutationResultDto> {
    return parseResourceMutationResultDto(
      await invokeCommand<unknown>("update_function_signature", {
        projectInstanceId,
        functionPath,
        request,
      }),
    );
  }
}
