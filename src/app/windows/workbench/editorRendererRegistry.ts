import { createElement } from "react";
import { GraphDocumentEditor } from "@/modules/graph-editor/public";
import { ChartEditor } from "@/modules/chart/public";
import { MindFileEditor, DocFileEditor } from "@/modules/document-editor/public";
import { DatabaseEditorContent } from "@/modules/database-editor/public";
import { LocalizedCatalogTreeRow } from "@/modules/node-catalog/public";
import type { EditorPanelScope, EditorRendererRegistry } from "@/modules/workbench/public";

function EventGraphEditor(props: EditorPanelScope<"event_graph">) {
  return createElement(GraphDocumentEditor, {
    ...props,
    catalogRowRenderer: LocalizedCatalogTreeRow,
  });
}

function FunctionGraphEditor(props: EditorPanelScope<"function_graph">) {
  return createElement(GraphDocumentEditor, {
    ...props,
    catalogRowRenderer: LocalizedCatalogTreeRow,
  });
}

function DatabaseEditorRenderer({ resourceRef }: EditorPanelScope<"database">) {
  return createElement(DatabaseEditorContent, { databaseId: resourceRef });
}

export const editorRendererRegistry = {
  event_graph: EventGraphEditor,
  function_graph: FunctionGraphEditor,
  chart: ChartEditor,
  mind: MindFileEditor,
  doc: DocFileEditor,
  database: DatabaseEditorRenderer,
} satisfies EditorRendererRegistry;
