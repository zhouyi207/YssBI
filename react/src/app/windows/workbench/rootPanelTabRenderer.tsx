import { useCallback, useMemo, useSyncExternalStore } from "react";
import { useTranslation } from "react-i18next";

import { buildEditorPanelTabMenu } from "@/features/application/editor/editorPanelTabMenu";
import { requestCloseEditorPanel } from "@/features/application/editor/editorPanelCloseCommands";
import { requestCloseWorkbenchPanel } from "@/features/application/editor/workbenchPanelClose";
import { useEditorPanelDirty } from "@/features/application/editor/useEditorPanelDirty";
import {
  RootPanelTabRenderer,
  showWorkbenchLayoutError,
  workbenchLayoutControl,
  workbenchLayoutRead,
  type RootPanelTabActions,
  type RootPanelTabComponent,
  type WorkbenchTabTarget,
} from "@/modules/workbench/public";

const WorkbenchRootPanelTabRenderer: RootPanelTabComponent = (props) => {
  const { t } = useTranslation();
  const metadata = props.params.metadata;
  const dirty = useEditorPanelDirty(metadata.role === "editor" ? metadata : null);
  const subscribe = useCallback(
    (listener: () => void) => workbenchLayoutRead.subscribePanel(props.panelInstanceId, listener),
    [props.panelInstanceId],
  );
  const getContentCollapsed = () => {
    const panel = workbenchLayoutRead.getPanel(props.panelInstanceId);
    return panel?.location.type === "edge"
      ? workbenchLayoutRead.getEdgeState(panel.location.position).collapsed
      : undefined;
  };
  const contentCollapsed = useSyncExternalStore(
    subscribe,
    getContentCollapsed,
    getContentCollapsed,
  );

  const requestClose = useCallback((target: WorkbenchTabTarget) => {
    if (target.metadata.role === "editor") void requestCloseEditorPanel(target.panelInstanceId);
    else void requestCloseWorkbenchPanel(target.panelInstanceId);
  }, []);

  const requestToggleContent = useCallback((target: WorkbenchTabTarget) => {
    const panel = workbenchLayoutRead.getPanel(target.panelInstanceId);
    if (panel?.groupId !== target.groupId || panel.location.type !== "edge") return;
    const edge = workbenchLayoutRead.getEdgeState(panel.location.position);
    const operation = edge.collapsed
      ? workbenchLayoutControl.reveal(panel.panelInstanceId)
      : workbenchLayoutControl.setEdgeCollapsed(panel.location.position, true);
    void operation.catch(showWorkbenchLayoutError);
  }, []);

  const buildEditorContextMenu = useCallback(
    (target: WorkbenchTabTarget) =>
      buildEditorPanelTabMenu(
        { panelInstanceId: target.panelInstanceId, groupId: target.groupId },
        t,
      ),
    [t],
  );

  const actions = useMemo<RootPanelTabActions>(
    () => ({ requestClose, requestToggleContent, buildEditorContextMenu }),
    [buildEditorContextMenu, requestClose, requestToggleContent],
  );

  return (
    <RootPanelTabRenderer
      {...props}
      dirty={dirty}
      contentCollapsed={contentCollapsed}
      actions={actions}
    />
  );
};

export const rootPanelTabRenderer = WorkbenchRootPanelTabRenderer;
