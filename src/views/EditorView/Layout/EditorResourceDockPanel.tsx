import { useEffect, useState } from "react";
import type { IDockviewPanelProps } from "dockview-react";

import type { WorkbenchPanelParams } from "@/features/core/dockview";
import { useVisibleGraphPanel } from "@/features/application/editor/useVisibleGraphPanel";
import { GroupContext } from "@/features/application/editor/editorGroupContext";
import { GraphEditor } from "../Canvas/core/GraphEditor";
import { WorksheetEditor } from "../Worksheet/WorksheetEditor";

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

export function EditorResourceDockPanel(props: IDockviewPanelProps<WorkbenchPanelParams>) {
  const groupId = useLivePanelGroupId(props.api);
  const metadata = props.params.metadata;
  if (metadata.role !== "editor") return null;

  const isWorksheet = metadata.resourceKind === "worksheet";

  const panel = (
    <div
      className="h-full min-h-0 w-full min-w-0 overflow-hidden bg-(--workbench-bg)"
      data-workbench-editor-panel
      data-panel-instance-id={props.api.id}
    >
      {isWorksheet ? (
        <WorksheetEditor key={`${metadata.resourceKind}:${metadata.resourceRef}`} />
      ) : (
        <GraphEditorPanel
          api={props.api}
          key={`${metadata.resourceKind}:${metadata.resourceRef}`}
          panelInstanceId={props.api.id}
          groupId={groupId}
          graphPath={metadata.resourceRef}
          graphKind={metadata.resourceKind}
        />
      )}
    </div>
  );

  return isWorksheet ? (
    <GroupContext.Provider value={groupId}>{panel}</GroupContext.Provider>
  ) : (
    panel
  );
}

function GraphEditorPanel({
  api,
  panelInstanceId,
  groupId,
  graphPath,
  graphKind,
}: {
  api: IDockviewPanelProps<WorkbenchPanelParams>["api"];
  panelInstanceId: string;
  groupId: string;
  graphPath: string;
  graphKind: "event" | "function";
}) {
  useVisibleGraphPanel(api, { groupId, graphPath });
  return (
    <GraphEditor
      panelInstanceId={panelInstanceId}
      groupId={groupId}
      graphPath={graphPath}
      graphKind={graphKind}
    />
  );
}
