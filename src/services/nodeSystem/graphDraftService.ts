import { invokeCommand } from "@/services/ipc";
import type {
  EditorGraphMutationDto,
  GraphDocumentDto,
  GraphDraftSaveDto,
  GraphDraftUpdateDto,
} from "@/shared/types/dto/editorMutation";
import {
  parseGraphDraftSaveDto,
  parseGraphDraftUpdateDto,
} from "@/shared/types/dto/editorMutationWireParser";

export class GraphDraftService {
  static async transform(
    projectInstanceId: string,
    graphPath: string,
    locale: string,
    document: GraphDocumentDto,
    mutation: EditorGraphMutationDto,
  ): Promise<GraphDraftUpdateDto> {
    const response: unknown = await invokeCommand("transform_graph_draft", {
      projectInstanceId,
      graphPath,
      locale,
      document,
      mutation,
    });
    return parseGraphDraftUpdateDto(response);
  }

  static async save(
    projectInstanceId: string,
    graphPath: string,
    locale: string,
    operationId: string,
    document: GraphDocumentDto,
  ): Promise<GraphDraftSaveDto> {
    const response: unknown = await invokeCommand("save_project_graph", {
      projectInstanceId,
      graphPath,
      locale,
      operationId,
      document,
    });
    return parseGraphDraftSaveDto(response, projectInstanceId);
  }
}
