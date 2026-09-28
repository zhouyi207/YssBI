import { invokeCommand } from "@/services/ipc";
import type { GraphEditVersionDto } from "@/shared/types/domain/editorMutation";

/** Shared node-graph editor residency; file operations belong to their file-type services. */
export class GraphService {
  static async unloadProjectGraph(
    graphPath: string,
    lifecycleToken: number,
    projectInstanceId: string,
    discardVersion?: GraphEditVersionDto,
  ): Promise<boolean> {
    return invokeCommand<boolean>("unload_project_graph", {
      graphPath,
      lifecycleToken,
      projectInstanceId,
      discardVersion,
    });
  }
}
