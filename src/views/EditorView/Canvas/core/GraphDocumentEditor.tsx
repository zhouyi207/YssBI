import { memo } from "react";
import { GraphCanvasController } from "./GraphCanvasController";
import { useIsActiveEditorPanel } from "@/features/application/editor";
import { CanvasDropZone } from "./CanvasDropZone";
import { useGraphRead } from "@/features/core/graph/read";
import { useProjectProjection } from "@/features/application/project/projectProjection";
import { resourceKey } from "@/features/core/resource";
import { useResourceRead } from "@/features/core/resource/read";
import type { EditorPanelScope } from "@/modules/workbench/public";
import { useVisibleGraphPanel } from "@/features/application/editor/useVisibleGraphPanel";

export type GraphDocumentEditorProps = EditorPanelScope<"event" | "function">;

/**
 * Graph editor shell per Dockview panel.
 * Each panel renders its own resource; only the physically active panel is interactive.
 */
export const GraphDocumentEditor = memo(function GraphDocumentEditor({
  panelInstanceId,
  groupId,
  resourceRef: graphPath,
  resourceKind: graphKind,
  isVisible,
}: GraphDocumentEditorProps) {
  useVisibleGraphPanel(isVisible, { groupId, graphPath });
  const mode = useIsActiveEditorPanel(panelInstanceId) ? "interactive" : "preview";
  const { graphLoadStatus: graphLoads } = useProjectProjection();
  const graphLoadStatus = graphLoads[graphPath];
  const graphProjectionReady = useGraphRead((snapshot) =>
    Boolean(snapshot.graphEntities[graphPath]),
  );
  const graphDocument = useResourceRead(
    (snapshot) => snapshot.documents[resourceKey({ id: graphPath, kind: graphKind })],
  );
  const graphReady =
    graphProjectionReady &&
    graphDocument?.loaded === true &&
    graphDocument.stale === false &&
    graphDocument.conflict === false &&
    graphLoadStatus !== "loading" &&
    graphLoadStatus !== "error";
  const graphUnavailable = graphLoadStatus === "error" || graphDocument?.conflict === true;

  return (
    <div className="flex flex-col w-full h-full min-h-0 min-w-0 overflow-hidden">
      <div className="flex-1 relative min-h-0 min-w-0 overflow-hidden">
        <CanvasDropZone
          panelInstanceId={panelInstanceId}
          groupId={groupId}
          graphPath={graphPath}
          graphKind={graphKind}
          mode={mode}
        >
          {graphReady ? (
            <GraphCanvasController
              mode={mode}
              panelInstanceId={panelInstanceId}
              groupId={groupId}
              graphPath={graphPath}
              graphKind={graphKind}
            />
          ) : graphUnavailable ? (
            <div className="absolute inset-0" role="alert" data-graph-load-error />
          ) : (
            <div className="absolute inset-0" aria-busy="true" data-graph-loading />
          )}
        </CanvasDropZone>
      </div>
    </div>
  );
});

export default GraphDocumentEditor;
