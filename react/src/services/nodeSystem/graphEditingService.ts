import type {
  EditorGraphMutationDto,
  GraphEditVersionDto,
  GraphSaveResultDto,
  GraphEditResultDto,
} from "@/shared/types/dto/editorMutation";
import { parseGraphSaveResultDto } from "@/shared/types/dto/editorMutationWireParser";
import { invokeGraphSync } from "./graphEditorSync";

export class GraphEditingService {
  static async resolve(
    projectInstanceId: string,
    graphPath: string,
    locale: string,
    version: GraphEditVersionDto,
  ): Promise<GraphEditResultDto> {
    const binding = { projectInstanceId, graphPath, locale };
    const result = await invokeGraphSync("resolve_editor_graph", { ...binding, version }, binding);
    return { ...result.data, changed: result.changed };
  }
  static async transform(
    projectInstanceId: string,
    graphPath: string,
    locale: string,
    version: GraphEditVersionDto,
    mutation: EditorGraphMutationDto,
  ): Promise<GraphEditResultDto> {
    const binding = { projectInstanceId, graphPath, locale };
    const result = await invokeGraphSync(
      "edit_graph",
      { ...binding, version, mutation, operationId: crypto.randomUUID() },
      binding,
    );
    return { ...result.data, changed: result.changed };
  }
  static async save(
    projectInstanceId: string,
    graphPath: string,
    locale: string,
    operationId: string,
    version: GraphEditVersionDto,
  ): Promise<GraphSaveResultDto> {
    const binding = { projectInstanceId, graphPath, locale };
    const result = await invokeGraphSync(
      "save_project_graph",
      { ...binding, version, operationId },
      binding,
    );
    return parseGraphSaveResultDto(
      {
        projectInstanceId,
        resourceRevision: result.resourceRevision,
        document: result.data.document,
        editing: result.data.editing,
        resultState: result.data.resultState,
        projectionReplacement: {
          graphPath,
          projection: result.data.projection,
          ...(result.functionEditorProjection === null
            ? {}
            : { functionEditorProjection: result.functionEditorProjection }),
        },
      },
      projectInstanceId,
    );
  }
  static async history(
    projectInstanceId: string,
    graphPath: string,
    locale: string,
    version: GraphEditVersionDto,
    redo: boolean,
  ): Promise<GraphEditResultDto> {
    const binding = { projectInstanceId, graphPath, locale };
    const result = await invokeGraphSync(
      "change_graph_history",
      { ...binding, version, redo, operationId: crypto.randomUUID() },
      binding,
    );
    return { ...result.data, changed: result.changed };
  }
}
