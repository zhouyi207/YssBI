import { invokeCommand } from "@/services/ipc";
import type { ClipboardSubgraphDto } from "@/shared/types/dto/clipboardSubgraph";
import type { GraphDocumentDto } from "@/shared/types/dto/editorMutation";
import { parseClipboardSubgraphDto } from "@/shared/types/dto/clipboardSubgraphWireParser";

export class GraphSubgraphService {
  static async exportSubgraph(
    projectInstanceId: string,
    graphPath: string,
    document: GraphDocumentDto,
    nodeIds: string[],
  ): Promise<ClipboardSubgraphDto> {
    const response: unknown = await invokeCommand("export_graph_subgraph", {
      projectInstanceId,
      graphPath,
      document,
      nodeIds,
    });
    return parseClipboardSubgraphDto(response);
  }
}
