import { invoke } from '@tauri-apps/api/core';
import type { EditorGraphProjectionDto } from '@/shared/types/dto/editorProjection';
import { parseEditorGraphProjectionDto } from '@/shared/types/dto/editorProjectionParser';

export class GraphProjectionService {
  static async loadGraph(
    graphPath: string,
    locale: string,
    lifecycleToken: number,
    projectInstanceId: string,
  ): Promise<EditorGraphProjectionDto> {
    const response: unknown = await invoke('load_project_graph', {
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
    const response: unknown = await invoke('hydrate_editor_graph', {
      projectInstanceId,
      graphPath,
      locale,
    });
    return parseEditorGraphProjectionDto(response);
  }
}
