import {
  useEffect,
  useState,
  type FunctionComponent,
} from 'react';
import type { IDockviewPanelProps } from 'dockview-react';

import {
  type WorkbenchComponentId,
  type WorkbenchPanelParams,
} from '@/features/core/dockview';
import { logsDockviewLayoutController } from '@/features/core/dockview/logsDockviewLayoutController';
import { useVisibleGraphPanel } from '@/features/application/editor/useVisibleGraphPanel';
import { GroupContext } from '@/features/core/editor';
import { LogWorkspaceDockview } from '@/views/LogView/LogWorkspaceDockview';
import { OutputPanel } from '@/views/LogView/OutputPanel';
import { DiagnosticsPanel } from '@/views/LogView/DiagnosticsPanel';
import { GraphEditor } from '../Canvas/core/GraphEditor';
import { WorksheetEditor } from '../Worksheet/WorksheetEditor';
import { DetailsPane } from './Detail/DetailsPane';
import { InspectPane } from './Detail/InspectPane';
import {
  WorkbenchCommandsPanel,
  WorkbenchDataPanel,
  WorkbenchNodesPanel,
  WorkbenchProjectPanel,
} from './WorkbenchActivityPanels';
import { ResultPanel } from './result/ResultPanel';

function useLivePanelGroupId(
  api: IDockviewPanelProps<WorkbenchPanelParams>['api'],
): string {
  const [groupId, setGroupId] = useState(() => api.group.id);

  useEffect(() => {
    const updateGroupId = () => setGroupId(api.group.id);
    const disposable = api.onDidGroupChange(updateGroupId);
    updateGroupId();
    return () => disposable.dispose();
  }, [api]);

  return groupId;
}

export function WorkbenchEditorPanel(
  props: IDockviewPanelProps<WorkbenchPanelParams>,
) {
  const groupId = useLivePanelGroupId(props.api);
  const metadata = props.params.metadata;
  if (metadata.role !== 'editor') return null;

  const isWorksheet = metadata.resourceKind === 'worksheet';

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
  api: IDockviewPanelProps<WorkbenchPanelParams>['api'];
  panelInstanceId: string;
  groupId: string;
  graphPath: string;
  graphKind: 'event' | 'function';
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

function MainLogsPanel() {
  return (
    <LogWorkspaceDockview
      layout={{ kind: 'main', controller: logsDockviewLayoutController }}
    />
  );
}

type WorkbenchDockviewComponent = FunctionComponent<
  IDockviewPanelProps<WorkbenchPanelParams>
>;

export const workbenchDockviewComponents = {
  GraphEditor: WorkbenchEditorPanel,
  WorksheetEditor: WorkbenchEditorPanel,
  Project: WorkbenchProjectPanel,
  Nodes: WorkbenchNodesPanel,
  Data: WorkbenchDataPanel,
  Commands: WorkbenchCommandsPanel,
  Details: DetailsPane,
  Inspect: InspectPane,
  Result: ResultPanel,
  Logs: MainLogsPanel,
  Output: OutputPanel,
  Diagnostics: DiagnosticsPanel,
} satisfies Record<WorkbenchComponentId, WorkbenchDockviewComponent>;
