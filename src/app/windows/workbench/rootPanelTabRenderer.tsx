import { useCallback, useMemo } from "react";
import { useTranslation } from "react-i18next";

import { buildEditorPanelTabMenu } from "@/features/application/editor/editorPanelTabMenu";
import { requestCloseEditorPanel } from "@/features/application/editor/editorPanelCloseCommands";
import {
  requestCloseWorkbenchGroup,
  requestCloseWorkbenchPanel,
} from "@/features/application/editor/workbenchPanelClose";
import { useEditorPanelDirty } from "@/features/application/editor/useEditorPanelDirty";
import {
  RootPanelTabRenderer,
  type RootPanelTabActions,
  type RootPanelTabComponent,
  type WorkbenchTabTarget,
} from "@/modules/workbench/public";

const WorkbenchRootPanelTabRenderer: RootPanelTabComponent = (props) => {
  const { t } = useTranslation();
  const metadata = props.params.metadata;
  const dirty = useEditorPanelDirty(metadata.role === "editor" ? metadata : null);

  const requestClose = useCallback((target: WorkbenchTabTarget) => {
    if (target.metadata.role === "editor") void requestCloseEditorPanel(target.panelInstanceId);
    else void requestCloseWorkbenchPanel(target.panelInstanceId);
  }, []);

  const requestCloseGroup = useCallback((target: WorkbenchTabTarget) => {
    void requestCloseWorkbenchGroup(target.groupId);
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
    () => ({ requestClose, requestCloseGroup, buildEditorContextMenu }),
    [buildEditorContextMenu, requestClose, requestCloseGroup],
  );

  return <RootPanelTabRenderer {...props} dirty={dirty} actions={actions} />;
};

export const rootPanelTabRenderer = WorkbenchRootPanelTabRenderer;
