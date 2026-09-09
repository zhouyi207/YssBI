import { createElement } from "react";
import { GraphDocumentEditor } from "@/modules/graph-editor/public";
import { ChartEditor } from "@/modules/chart/public";
import { DatabaseEditorContent } from "@/modules/database-editor/public";
import { LocalizedCatalogTreeRow } from "@/modules/node-catalog/public";
import type { EditorPanelScope, EditorRendererRegistry } from "@/modules/workbench/public";

function GraphEditorRenderer(props: EditorPanelScope<"event" | "function">) {
  return createElement(GraphDocumentEditor, {
    ...props,
    catalogRowRenderer: LocalizedCatalogTreeRow,
  });
}

function DatabaseEditorRenderer({ resourceRef }: EditorPanelScope<"database">) {
  return createElement(DatabaseEditorContent, { databaseId: resourceRef });
}

export const editorRendererRegistry = {
  event: GraphEditorRenderer,
  function: GraphEditorRenderer,
  chart: ChartEditor,
  database: DatabaseEditorRenderer,
} satisfies EditorRendererRegistry;
