import type { ReactNode } from "react";
import type { RootPanelProps } from "./panelContribution";
import type { EditorRendererRegistry } from "./editorRenderer";

type EditorResourcePanelProps = RootPanelProps & {
  readonly rendererRegistry: EditorRendererRegistry;
};

export function EditorResourcePanel({ rendererRegistry, ...props }: EditorResourcePanelProps) {
  const groupId = props.groupId;
  const isVisible = props.visible;
  const metadata = props.params.metadata;
  if (metadata.role !== "editor") return null;

  const editorScope = {
    panelInstanceId: props.panelInstanceId,
    groupId,
    resourceRef: metadata.resourceRef,
    isVisible,
  };
  const editorKey = `${metadata.resourceKind}:${metadata.resourceRef}`;

  let editor: ReactNode;
  switch (metadata.resourceKind) {
    case "event": {
      const Editor = rendererRegistry.event;
      editor = <Editor key={editorKey} {...editorScope} resourceKind="event" />;
      break;
    }
    case "function": {
      const Editor = rendererRegistry.function;
      editor = <Editor key={editorKey} {...editorScope} resourceKind="function" />;
      break;
    }
    case "chart": {
      const Editor = rendererRegistry.chart;
      editor = <Editor key={editorKey} {...editorScope} resourceKind="chart" />;
      break;
    }
    case "database": {
      const Editor = rendererRegistry.database;
      editor = <Editor key={editorKey} {...editorScope} resourceKind="database" />;
      break;
    }
  }

  return (
    <div
      className="h-full min-h-0 w-full min-w-0 overflow-hidden bg-(--workbench-bg)"
      data-workbench-editor-panel
      data-panel-instance-id={props.panelInstanceId}
    >
      {editor}
    </div>
  );
}
