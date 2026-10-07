import { invokeCommand } from "@/services/ipc";
import type { ClipboardSubgraphDto } from "@/shared/types/dto/clipboardSubgraph";
import type { GraphEditVersionDto } from "@/shared/types/dto/editorMutation";
import { parseClipboardSubgraphDto } from "@/shared/types/dto/clipboardSubgraphWireParser";

export class GraphSubgraphService {
  static async exportSubgraph(
    projectInstanceId: string,
    graphPath: string,
    version: GraphEditVersionDto,
    nodeIds: string[],
  ): Promise<ClipboardSubgraphDto> {
    const response: unknown = await invokeCommand("export_graph_subgraph", {
      projectInstanceId,
      graphPath,
      version,
      nodeIds,
    });
    return parseClipboardSubgraphDto(response);
  }
}
