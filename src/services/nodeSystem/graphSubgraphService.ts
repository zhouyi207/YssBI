import { invokeCommand } from "@/services/ipc";
import type { ClipboardSubgraphDto } from "@/shared/types/dto/clipboardSubgraph";
import { parseClipboardSubgraphDto } from "@/shared/types/dto/clipboardSubgraphWireParser";

export class GraphSubgraphService {
  static async exportSubgraph(
    projectInstanceId: string,
    graphPath: string,
    nodeIds: string[],
  ): Promise<ClipboardSubgraphDto> {
    const response: unknown = await invokeCommand("export_graph_subgraph", {
      projectInstanceId,
      graphPath,
      nodeIds,
    });
    return parseClipboardSubgraphDto(response);
  }
}
