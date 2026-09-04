import { invokeCommand } from "@/services/ipc";
import type {
  CompileGraphDraftDto,
  EditorGraphMutationDto,
  GraphDocumentDto,
  GraphDraftSaveDto,
  GraphDraftTransformDto,
} from "@/shared/types/dto/editorMutation";
import {
  parseCompileGraphDraftDto,
  parseGraphDraftSaveDto,
  parseGraphDraftTransformDto,
} from "@/shared/types/dto/editorMutationWireParser";

export class GraphDraftService {
  static async compile(
    projectInstanceId: string,
    graphPath: string,
    locale: string,
    document: GraphDocumentDto,
  ): Promise<CompileGraphDraftDto> {
    const response: unknown = await invokeCommand("compile_graph_draft", {
      projectInstanceId,
      graphPath,
      locale,
      document,
    });
    return parseCompileGraphDraftDto(response);
  }

  static async transform(
    projectInstanceId: string,
    graphPath: string,
    locale: string,
    document: GraphDocumentDto,
    mutation: EditorGraphMutationDto,
  ): Promise<GraphDraftTransformDto> {
    const response: unknown = await invokeCommand("transform_graph_draft", {
      projectInstanceId,
      graphPath,
      locale,
      document,
      mutation,
    });
    return parseGraphDraftTransformDto(response);
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
