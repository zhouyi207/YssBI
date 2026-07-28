import { invoke } from '@tauri-apps/api/core';
import type {
  EditorGraphMutationDto,
  GraphMutationResultDto,
  MutationRequestDto,
} from '@/shared/types/dto/editorMutation';

export class GraphMutationService {
  static mutateGraph(
    graphPath: string,
    locale: string,
    request: MutationRequestDto<EditorGraphMutationDto>,
  ): Promise<GraphMutationResultDto> {
    return invoke<GraphMutationResultDto>('mutate_graph_document', {
      graphPath,
      locale,
      request,
    });
  }
}
