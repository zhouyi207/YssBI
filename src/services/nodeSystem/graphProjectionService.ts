import type { GraphEditorSessionDto } from "@/shared/types/dto/editorMutation";
import { invokeGraphSync } from "./graphEditorSync";

export class GraphProjectionService {
  static async loadGraph(
    graphPath: string,
    locale: string,
    lifecycleToken: number,
    projectInstanceId: string,
  ): Promise<GraphEditorSessionDto> {
    const binding = { projectInstanceId, graphPath, locale };
    return (await invokeGraphSync("load_project_graph", { ...binding, lifecycleToken }, binding))
      .data;
  }
  static async hydrateGraph(
    projectInstanceId: string,
    graphPath: string,
    locale: string,
  ): Promise<GraphEditorSessionDto> {
    const binding = { projectInstanceId, graphPath, locale };
    return (await invokeGraphSync("hydrate_editor_graph", binding, binding)).data;
  }
}
