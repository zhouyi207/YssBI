import { invokeCommand } from "@/services/ipc";
import type {
  HistoryMutationDto,
  HistoryStatusDto,
  MutationRequestDto,
  ResourceMutationResultDto,
} from "@/shared/types/dto/editorMutation";

export class HistoryService {
  static getStatus(projectInstanceId: string): Promise<HistoryStatusDto> {
    return invokeCommand<HistoryStatusDto>("get_project_history_status", { projectInstanceId });
  }

  static undo(
    projectInstanceId: string,
    locale: string,
    request: MutationRequestDto<HistoryMutationDto>,
  ): Promise<ResourceMutationResultDto> {
    return invokeCommand<ResourceMutationResultDto>("undo_graph_document", {
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
    return invokeCommand<ResourceMutationResultDto>("redo_graph_document", {
      projectInstanceId,
      locale,
      request,
    });
  }
}
