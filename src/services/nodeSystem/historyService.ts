import { invoke } from '@tauri-apps/api/core';
import type {
  HistoryMutationDto,
  HistoryStatusDto,
  MutationRequestDto,
  ResourceMutationResultDto,
} from '@/shared/types/dto/editorMutation';

export class HistoryService {
  static getStatus(projectInstanceId: string): Promise<HistoryStatusDto> {
    return invoke<HistoryStatusDto>('get_project_history_status', { projectInstanceId });
  }

  static undo(
    projectInstanceId: string,
    locale: string,
    request: MutationRequestDto<HistoryMutationDto>,
  ): Promise<ResourceMutationResultDto> {
    return invoke<ResourceMutationResultDto>('undo_graph_document', {
      projectInstanceId,
      locale,
      request,
    });
  }

  static redo(
    projectInstanceId: string,
    locale: string,
    request: MutationRequestDto<HistoryMutationDto>,
  ): Promise<ResourceMutationResultDto> {
    return invoke<ResourceMutationResultDto>('redo_graph_document', {
      projectInstanceId,
      locale,
      request,
    });
  }
}
