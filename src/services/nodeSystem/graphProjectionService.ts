import { invoke } from '@tauri-apps/api/core';
import type { EditorGraphProjectionDto } from '@/shared/types/dto/editorProjection';

export class GraphProjectionService {
  static loadGraph(
    graphPath: string,
    locale: string,
    lifecycleToken: number,
    projectInstanceId: string,
  ): Promise<EditorGraphProjectionDto> {
    return invoke<EditorGraphProjectionDto>('load_project_graph', {
      graphPath,
      locale,
      lifecycleToken,
      projectInstanceId,
    });
  }

  static hydrateGraph(
    projectInstanceId: string,
    graphPath: string,
    locale: string,
  ): Promise<EditorGraphProjectionDto> {
    return invoke<EditorGraphProjectionDto>('hydrate_editor_graph', {
      projectInstanceId,
      graphPath,
      locale,
    });
  }
}
