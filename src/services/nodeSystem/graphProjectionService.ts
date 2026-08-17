import { invokeCommand } from '@/services/ipc';
import type { EditorGraphProjectionDto } from '@/shared/types/dto/editorProjection';
import { parseEditorGraphProjectionDto } from '@/shared/types/dto/editorProjectionParser';

export class GraphProjectionService {
  static async loadGraph(
    graphPath: string,
    locale: string,
    lifecycleToken: number,
    projectInstanceId: string,
  ): Promise<EditorGraphProjectionDto> {
    const response: unknown = await invokeCommand('load_project_graph', {
      graphPath,
      locale,
      lifecycleToken,
      projectInstanceId,
    });
    return parseEditorGraphProjectionDto(response);
  }

  static async hydrateGraph(
    projectInstanceId: string,
    graphPath: string,
    locale: string,
  ): Promise<EditorGraphProjectionDto> {
    const response: unknown = await invokeCommand('hydrate_editor_graph', {
      projectInstanceId,
      graphPath,
      locale,
    });
    return parseEditorGraphProjectionDto(response);
  }
}
