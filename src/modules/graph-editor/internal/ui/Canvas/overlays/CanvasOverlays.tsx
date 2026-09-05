import { createPortal } from "react-dom";
import type { NodeCreationDescriptor } from "@/features/domain/nodeCatalog/creationDescriptor";
import type { PortAddressDto } from "@/shared/types/domain/editorProjection";
import { getOverlayPortalRoot } from "@/shared/ui/overlayPortalRoot";
import { NodePalette, type NodePaletteCatalogRowRenderer } from "../../NodePalette";
import { PinResultSearch } from "./PinResultSearchPalette";
import { CanvasExecutionToolbar } from "./CanvasExecutionToolbar";

export type CanvasOverlayGraphModel =
  | { kind: "event"; graphPath: string }
  | { kind: "function"; graphPath: string }
  | { kind: "unavailable" };

export type CanvasPaletteOverlayModel =
  | { kind: "hidden" }
  | {
      kind: "visible";
      x: number;
      y: number;
      graphPath: string | null;
      sourcePort: PortAddressDto | null;
      onSelect: (descriptor: NodeCreationDescriptor, locale: string) => void;
      onClose: () => void;
    };

export type CanvasExecutionOverlayModel =
  | { kind: "hidden" }
  | {
      kind: "graph";
      graphPath: string;
      canExecute: boolean;
      executeUnavailableReason: "functionGraph" | "blockingProblems" | null;
      compileStatus: "uncompiled" | "compiling" | "compiled" | "blocked" | "failed";
      onCompile: () => void;
      onExecute: () => void;
      onCancelExecution: () => void;
      onClearArtifacts: () => void;
    };

export interface CanvasOverlaysModel {
  graph: CanvasOverlayGraphModel;
  palette: CanvasPaletteOverlayModel;
  execution: CanvasExecutionOverlayModel;
}

export default function CanvasOverlays({
  model,
  catalogRowRenderer,
}: {
  model: CanvasOverlaysModel;
  catalogRowRenderer: NodePaletteCatalogRowRenderer;
}) {
  const { graph, palette, execution } = model;

  return (
    <>
      {graph.kind === "event" ? (
        <div className="absolute left-3 top-3 z-40">
          <PinResultSearch graphPath={graph.graphPath} />
        </div>
      ) : null}

      {execution.kind === "graph" ? (
        <CanvasExecutionToolbar
          graphPath={execution.graphPath}
          canExecute={execution.canExecute}
          executeUnavailableReason={execution.executeUnavailableReason}
          compileStatus={execution.compileStatus}
          onCompile={execution.onCompile}
          onExecute={execution.onExecute}
          onCancelExecution={execution.onCancelExecution}
          onClearArtifacts={execution.onClearArtifacts}
        />
      ) : null}

      {palette.kind === "visible"
        ? createPortal(
            <NodePalette
              x={palette.x}
              y={palette.y}
              graphPath={palette.graphPath}
              sourcePort={palette.sourcePort}
              catalogRowRenderer={catalogRowRenderer}
              onSelect={palette.onSelect}
              onClose={palette.onClose}
            />,
            getOverlayPortalRoot(),
          )
        : null}
    </>
  );
}
