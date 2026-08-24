import {
  useEffect,
  useState,
  type FunctionComponent,
} from 'react';
import type { IDockviewPanelProps } from 'dockview-react';

import {
  layoutTabFromEditorMetadata,
  type WorkbenchComponentId,
  type WorkbenchPanelParams,
} from '@/features/core/dockview';
import { logsDockviewLayoutController } from '@/features/core/dockview/logsDockviewLayoutController';
import { GroupContext } from '@/features/core/editor';
import { LogWorkspaceDockview } from '@/views/LogView/LogWorkspaceDockview';
import { OutputPanel } from '@/views/LogView/OutputPanel';
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

  const layoutTab = layoutTabFromEditorMetadata(metadata);
  const EditorComponent = layoutTab.component === 'WorksheetEditor'
    ? WorksheetEditor
    : GraphEditor;

  return (
    <GroupContext.Provider value={groupId}>
      <div
        className="h-full min-h-0 w-full min-w-0 overflow-hidden bg-(--workbench-bg)"
        data-workbench-editor-panel
        data-panel-instance-id={props.api.id}
      >
        <EditorComponent key={`${layoutTab.type}:${layoutTab.id}`} />
      </div>
    </GroupContext.Provider>
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
} satisfies Record<WorkbenchComponentId, WorkbenchDockviewComponent>;
