import { createElement } from "react";
import { GraphDocumentEditor } from "@/modules/graph-editor/public";
import { ChartEditor } from "@/modules/chart/public";
import { LocalizedCatalogTreeRow } from "@/modules/node-catalog/public";
import type { EditorPanelScope, EditorRendererRegistry } from "@/modules/workbench/public";

function GraphEditorRenderer(props: EditorPanelScope<"event" | "function">) {
  return createElement(GraphDocumentEditor, {
    ...props,
    catalogRowRenderer: LocalizedCatalogTreeRow,
  });
}

export const editorRendererRegistry = {
  event: GraphEditorRenderer,
  function: GraphEditorRenderer,
  chart: ChartEditor,
} satisfies EditorRendererRegistry;
