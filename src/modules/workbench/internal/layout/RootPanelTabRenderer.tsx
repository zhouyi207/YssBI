import { useTranslation } from "react-i18next";
import {
  VscClose,
  VscCloseAll,
  VscDatabase,
  VscError,
  VscExtensions,
  VscGraphLine,
  VscInfo,
  VscInspect,
  VscLibrary,
  VscProject,
  VscOutput,
  VscPreview,
  VscSparkle,
  VscSymbolEvent,
  VscSymbolMethod,
  VscTerminal,
} from "react-icons/vsc";
import type { IconType } from "react-icons";
import {
  ActionMenu,
  usePositionedActionMenu,
  type ActionMenuSection,
} from "@/shared/ui/actionMenu";
import type { WorkbenchPanelMetadata, WorkbenchViewId } from "./workbenchPanelModel";
import type { RootPanelProps } from "./panelContribution";
import {
  isWorkbenchActivityViewId,
  isWorkbenchPersistentViewMetadata,
} from "./workbenchPanelModel";

export interface WorkbenchTabTarget {
  readonly panelInstanceId: string;
  readonly groupId: string;
  readonly metadata: WorkbenchPanelMetadata;
}
export interface RootPanelTabActions {
  readonly requestClose: (target: WorkbenchTabTarget) => void;
  readonly requestCloseGroup: (target: WorkbenchTabTarget) => void;
  readonly buildEditorContextMenu: (target: WorkbenchTabTarget) => ActionMenuSection[];
}
export interface RootPanelTabRendererProps extends RootPanelProps {
  readonly dirty: boolean;
  readonly actions: RootPanelTabActions;
}
const VIEW_ICONS: Record<WorkbenchViewId, IconType> = {
  project: VscProject,
  nodes: VscLibrary,
  commands: VscTerminal,
  plugins: VscExtensions,
  details: VscInfo,
  assistant: VscSparkle,
  inspect: VscInspect,
  logs: VscTerminal,
  output: VscOutput,
  problems: VscError,
};
const VIEW_TITLE_KEYS: Record<WorkbenchViewId, string> = {
  project: "activityBar.project",
  nodes: "activityBar.nodes",
  commands: "activityBar.commands",
  plugins: "activityBar.plugins",
  details: "panel.details",
  assistant: "panel.assistant",
  inspect: "panel.inspect",
  logs: "panel.logs",
  output: "panel.output",
  problems: "panel.problems",
};

export function RootPanelTabRenderer({ dirty, actions, ...props }: RootPanelTabRendererProps) {
  const { t } = useTranslation();
  const metadata = props.params.metadata;
  const target: WorkbenchTabTarget = {
    panelInstanceId: props.panelInstanceId,
    groupId: props.groupId,
    metadata,
  };
  const { contextMenu, setContextMenu, closeActionMenu } =
    usePositionedActionMenu<WorkbenchTabTarget>();
  const Icon =
    metadata.role === "view"
      ? VIEW_ICONS[metadata.viewId]
      : metadata.role === "result"
        ? VscPreview
        : metadata.role === "plugin"
          ? VscExtensions
          : (
              {
                event: VscSymbolEvent,
                function: VscSymbolMethod,
                chart: VscGraphLine,
                database: VscDatabase,
              } as const
            )[metadata.resourceKind];
  const title = metadata.role === "view" ? t(VIEW_TITLE_KEYS[metadata.viewId]) : props.title;
  const sections: ActionMenuSection[] = contextMenu
    ? contextMenu.target.metadata.role === "editor"
      ? actions.buildEditorContextMenu(contextMenu.target)
      : [
          {
            items: [
              {
                id: "close",
                label: t("tabBar.contextMenu.close"),
                icon: <VscClose size={12} />,
                onClick: () => actions.requestClose(contextMenu.target),
              },
              {
                id: "close-group",
                label: t("tabBar.closeGroup"),
                icon: <VscCloseAll size={12} />,
                onClick: () => actions.requestCloseGroup(contextMenu.target),
              },
            ],
          },
        ]
    : [];
  return (
    <>
      <span
        className="workbench-tab-label"
        title={title}
        data-panel-instance-id={props.panelInstanceId}
        onContextMenu={(event) => {
          event.preventDefault();
          event.stopPropagation();
          if (
            !isWorkbenchPersistentViewMetadata(metadata) &&
            !(metadata.role === "view" && isWorkbenchActivityViewId(metadata.viewId))
          )
            setContextMenu({ x: event.clientX, y: event.clientY, target });
        }}
      >
        <Icon size={15} aria-hidden />
        <span className="truncate">{title}</span>
        {dirty ? <span className="workbench-tab-dirty" aria-label={t("common.unsaved")} /> : null}
      </span>
      {contextMenu ? (
        <ActionMenu
          position={{ x: contextMenu.x, y: contextMenu.y }}
          sections={sections}
          onClose={closeActionMenu}
        />
      ) : null}
    </>
  );
}
