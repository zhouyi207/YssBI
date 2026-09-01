import { GraphDocumentEditor } from "@/views/EditorView/Canvas/core/GraphDocumentEditor";
import { ChartEditor } from "@/modules/chart/public";
import type { EditorRendererRegistry } from "@/modules/workbench/public";

export const editorRendererRegistry = {
  event: GraphDocumentEditor,
  function: GraphDocumentEditor,
  chart: ChartEditor,
} satisfies EditorRendererRegistry;
