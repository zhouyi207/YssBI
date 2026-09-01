import { GraphDocumentEditor } from "@/views/EditorView/Canvas/core/GraphDocumentEditor";
import { ChartEditor } from "@/views/EditorView/Chart/ChartEditor";
import type { EditorRendererRegistry } from "@/views/EditorView/Layout/editorRenderer";

export const editorRendererRegistry = {
  event: GraphDocumentEditor,
  function: GraphDocumentEditor,
  chart: ChartEditor,
} satisfies EditorRendererRegistry;
