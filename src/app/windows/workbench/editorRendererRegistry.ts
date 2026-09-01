import { GraphDocumentEditor } from "@/modules/graph-editor/public";
import { ChartEditor } from "@/modules/chart/public";
import type { EditorRendererRegistry } from "@/modules/workbench/public";

export const editorRendererRegistry = {
  event: GraphDocumentEditor,
  function: GraphDocumentEditor,
  chart: ChartEditor,
} satisfies EditorRendererRegistry;
