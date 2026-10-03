import { invokeCommand } from "@/services/ipc";
import { z } from "zod";
import type { GraphEditVersionDto } from "@/shared/types/domain/editorMutation";

const graphUnloadAcknowledgement = z.boolean();

/** Shared node-graph editor residency; file operations belong to their file-type services. */
export class GraphService {
  static async unloadProjectGraph(
    graphPath: string,
    lifecycleToken: number,
    projectInstanceId: string,
    discardVersion?: GraphEditVersionDto,
  ): Promise<boolean> {
    return graphUnloadAcknowledgement.parse(
      await invokeCommand<unknown>("unload_project_graph", {
        graphPath,
        lifecycleToken,
        projectInstanceId,
        discardVersion,
      }),
    );
  }
}
