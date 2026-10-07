import type { ComponentType } from "react";
import type { RootPanelProps } from "./panelContribution";
import type { EditorPanelScope, EditorRendererRegistry } from "./editorRenderer";

type EditorResourcePanelProps = RootPanelProps & {
  readonly rendererRegistry: EditorRendererRegistry;
};

function renderEditor<Kind extends EditorPanelScope["resourceKind"]>(
  rendererRegistry: EditorRendererRegistry,
  scope: EditorPanelScope<Kind>,
) {
  const Editor: ComponentType<EditorPanelScope<Kind>> = rendererRegistry[scope.resourceKind];
  return <Editor key={`${scope.resourceKind}:${scope.resourceRef}`} {...scope} />;
}

export function EditorResourcePanel({ rendererRegistry, ...props }: EditorResourcePanelProps) {
  const groupId = props.groupId;
  const isVisible = props.visible;
  const metadata = props.params.metadata;
  if (metadata.role !== "editor") return null;

  const editorScope = {
    panelInstanceId: props.panelInstanceId,
    groupId,
    resourceRef: metadata.resourceRef,
    resourceKind: metadata.resourceKind,
    isVisible,
  };

  return (
    <div
      className="h-full min-h-0 w-full min-w-0 overflow-hidden bg-(--workbench-bg)"
      data-workbench-editor-panel
      data-panel-instance-id={props.panelInstanceId}
    >
      {renderEditor(rendererRegistry, editorScope)}
    </div>
  );
}
