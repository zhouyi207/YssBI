import { invoke } from '@tauri-apps/api/core';
import type {
  EditorGraphMutationDto,
  GraphMutationResultDto,
  MutationRequestDto,
} from '@/shared/types/dto/editorMutation';
import {
  parseEditorGraphMutationDto,
  parseGraphMutationResultDto,
} from '@/shared/types/dto/editorMutationWireParser';

export class GraphMutationService {
  static async mutateGraph(
    projectInstanceId: string,
    graphPath: string,
    locale: string,
    request: MutationRequestDto<EditorGraphMutationDto>,
  ): Promise<GraphMutationResultDto> {
    const wireRequest = request.payload.type === 'insertReroute'
      ? { ...request, payload: parseEditorGraphMutationDto(request.payload) }
      : request;
    const response: unknown = await invoke('mutate_graph_document', {
      projectInstanceId,
      graphPath,
      locale,
      request: wireRequest,
    });
    return parseGraphMutationResultDto(response, projectInstanceId);
  }
}
