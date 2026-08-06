import { invoke } from '@tauri-apps/api/core';
import type {
  EditorGraphMutationDto,
  GraphMutationResultDto,
  MutationRequestDto,
} from '@/shared/types/dto/editorMutation';
import { parseGraphMutationResultDto } from '@/shared/types/dto/editorMutationWireParser';

export class GraphMutationService {
  static async mutateGraph(
    projectInstanceId: string,
    graphPath: string,
    locale: string,
    request: MutationRequestDto<EditorGraphMutationDto>,
  ): Promise<GraphMutationResultDto> {
    const response: unknown = await invoke('mutate_graph_document', {
      projectInstanceId,
      graphPath,
      locale,
      request,
    });
    return parseGraphMutationResultDto(response);
  }
}
