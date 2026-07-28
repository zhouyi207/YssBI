import { invoke } from '@tauri-apps/api/core';
import type {
  HistoryMutationDto,
  HistoryStatusDto,
  MutationRequestDto,
  ResourceMutationResultDto,
} from '@/shared/types/dto/editorMutation';

export class HistoryService {
  static getStatus(): Promise<HistoryStatusDto> {
    return invoke<HistoryStatusDto>('get_project_history_status');
  }

  static undo(
    locale: string,
    request: MutationRequestDto<HistoryMutationDto>,
  ): Promise<ResourceMutationResultDto> {
    return invoke<ResourceMutationResultDto>('undo_graph_document', { locale, request });
  }

  static redo(
    locale: string,
    request: MutationRequestDto<HistoryMutationDto>,
  ): Promise<ResourceMutationResultDto> {
    return invoke<ResourceMutationResultDto>('redo_graph_document', { locale, request });
  }
}
