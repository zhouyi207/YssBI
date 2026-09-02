import { invokeCommand } from "@/services/ipc";
import type { GraphEditorSessionDto } from "@/shared/types/dto/editorMutation";
import { parseGraphEditorSessionDto } from "@/shared/types/dto/editorMutationWireParser";

export class GraphProjectionService {
  static async loadGraph(
    graphPath: string,
    locale: string,
    lifecycleToken: number,
    projectInstanceId: string,
  ): Promise<GraphEditorSessionDto> {
    const response: unknown = await invokeCommand("load_project_graph", {
      graphPath,
      locale,
      lifecycleToken,
      projectInstanceId,
    });
    return parseGraphEditorSessionDto(response);
  }

  static async hydrateGraph(
    projectInstanceId: string,
    graphPath: string,
    locale: string,
  ): Promise<GraphEditorSessionDto> {
    const response: unknown = await invokeCommand("hydrate_editor_graph", {
      projectInstanceId,
      graphPath,
      locale,
    });
    return parseGraphEditorSessionDto(response);
  }
}
