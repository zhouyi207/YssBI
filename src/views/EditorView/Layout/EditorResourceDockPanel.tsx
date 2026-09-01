import { useEffect, useState, type ReactNode } from "react";
import type { IDockviewPanelProps } from "dockview-react";

import type { WorkbenchPanelParams } from "@/features/core/dockview";
import type { EditorRendererRegistry } from "./editorRenderer";

function useLivePanelGroupId(api: IDockviewPanelProps<WorkbenchPanelParams>["api"]): string {
  const [groupId, setGroupId] = useState(() => api.group.id);

  useEffect(() => {
    const updateGroupId = () => setGroupId(api.group.id);
    const disposable = api.onDidGroupChange(updateGroupId);
    updateGroupId();
    return () => disposable.dispose();
  }, [api]);

  return groupId;
}

function useLivePanelVisibility(api: IDockviewPanelProps<WorkbenchPanelParams>["api"]): boolean {
  const [isVisible, setIsVisible] = useState(() => api.isVisible);

  useEffect(() => {
    const updateVisibility = () => setIsVisible(api.isVisible);
    const visibilityDisposable = api.onDidVisibilityChange(updateVisibility);
    const groupDisposable = api.onDidGroupChange(updateVisibility);
    updateVisibility();
    return () => {
      visibilityDisposable.dispose();
      groupDisposable.dispose();
    };
  }, [api]);

  return isVisible;
}

type EditorResourceDockPanelProps = IDockviewPanelProps<WorkbenchPanelParams> & {
  readonly rendererRegistry: EditorRendererRegistry;
};

export function EditorResourceDockPanel({
  rendererRegistry,
  ...props
}: EditorResourceDockPanelProps) {
  const groupId = useLivePanelGroupId(props.api);
  const isVisible = useLivePanelVisibility(props.api);
  const metadata = props.params.metadata;
  if (metadata.role !== "editor") return null;

  const editorScope = {
    panelInstanceId: props.api.id,
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
  }

  return (
    <div
      className="h-full min-h-0 w-full min-w-0 overflow-hidden bg-(--workbench-bg)"
      data-workbench-editor-panel
      data-panel-instance-id={props.api.id}
    >
      {editor}
    </div>
  );
}
