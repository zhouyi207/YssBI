import { useTranslation } from "react-i18next";
import {
  VscClose,
  VscDatabase,
  VscError,
  VscExtensions,
  VscEye,
  VscEyeClosed,
  VscGraphLine,
  VscInfo,
  VscLibrary,
  VscLayoutPanelCenter,
  VscMultipleWindows,
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
import { canFloatWorkbenchPanel, hasWorkbenchPanelCloseButton } from "./workbenchActivityGroup";
import { workbenchLayoutRead } from "./workbenchRead";
import { workbenchLayoutControl } from "./workbenchControl";
import { showWorkbenchLayoutError } from "../application/workbenchLayoutErrorFeedback";
import { isWorkbenchActivityViewId } from "./workbenchPanelModel";

export interface WorkbenchTabTarget {
  readonly panelInstanceId: string;
  readonly groupId: string;
  readonly metadata: WorkbenchPanelMetadata;
}
export interface RootPanelTabActions {
  readonly requestClose: (target: WorkbenchTabTarget) => void;
  readonly requestToggleContent: (target: WorkbenchTabTarget) => void;
  readonly buildEditorContextMenu: (target: WorkbenchTabTarget) => ActionMenuSection[];
}
export interface RootPanelTabRendererProps extends RootPanelProps {
  readonly dirty: boolean;
  readonly contentCollapsed?: boolean;
  readonly actions: RootPanelTabActions;
}
const VIEW_ICONS: Record<WorkbenchViewId, IconType> = {
  project: VscProject,
  nodes: VscLibrary,
  commands: VscTerminal,
  plugins: VscExtensions,
  details: VscInfo,
  assistant: VscSparkle,
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
  logs: "panel.logs",
  output: "panel.output",
  problems: "panel.problems",
};

export function RootPanelTabRenderer({
  dirty,
  contentCollapsed,
  actions,
  ...props
}: RootPanelTabRendererProps) {
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
                disabled: !hasWorkbenchPanelCloseButton(contextMenu.target.metadata),
                onClick: () => actions.requestClose(contextMenu.target),
              },
              ...(contentCollapsed !== undefined
                ? [
                    {
                      id: "toggle-content",
                      label: t(
                        contentCollapsed
                          ? "tabBar.contextMenu.expandContent"
                          : "tabBar.contextMenu.hideContent",
                      ),
                      icon: contentCollapsed ? <VscEye size={12} /> : <VscEyeClosed size={12} />,
                      onClick: () => actions.requestToggleContent(contextMenu.target),
                    },
                  ]
                : []),
            ],
          },
        ]
    : [];
  const menuPanel = contextMenu && workbenchLayoutRead.getPanel(contextMenu.target.panelInstanceId);
  if (
    menuPanel &&
    menuPanel.location.type !== "edge" &&
    canFloatWorkbenchPanel(menuPanel.metadata)
  ) {
    const location = menuPanel.location;
    const groupPanels = workbenchLayoutRead.listGroupPanels(menuPanel.groupId);
    const floatTab = (label: string) => ({
      id: "float-tab",
      label,
      icon: <VscMultipleWindows size={12} aria-hidden />,
      onClick: () =>
        void workbenchLayoutControl
          .floatPanel(menuPanel.panelInstanceId)
          .catch(showWorkbenchLayoutError),
    });
    const floatGroup = (label: string) => ({
      id: "float-group",
      label,
      icon: <VscMultipleWindows size={12} aria-hidden />,
      disabled: !groupPanels.every((panel) => canFloatWorkbenchPanel(panel.metadata)),
      onClick: () =>
        void workbenchLayoutControl.floatGroup(menuPanel.groupId).catch(showWorkbenchLayoutError),
    });
    if (location.type === "float") {
      const windowPanelCount = workbenchLayoutRead
        .listPanels()
        .filter(
          (panel) =>
            panel.location.type === "float" && panel.location.layoutId === location.layoutId,
        ).length;
      sections.push({
        items: [
          ...(windowPanelCount > 1 ? [floatTab(t("tabBar.contextMenu.detachTab"))] : []),
          ...(windowPanelCount > groupPanels.length && groupPanels.length > 1
            ? [floatGroup(t("tabBar.contextMenu.detachGroup"))]
            : []),
          {
            id: "dock-float",
            label: t("tabBar.contextMenu.dockFloat"),
            icon: <VscLayoutPanelCenter size={12} aria-hidden />,
            onClick: () =>
              void workbenchLayoutControl
                .dockFloat(location.layoutId)
                .catch(showWorkbenchLayoutError),
          },
        ],
      });
    } else {
      sections.push({
        items: [
          floatTab(t("tabBar.contextMenu.floatTab")),
          ...(groupPanels.length > 1 ? [floatGroup(t("tabBar.contextMenu.floatGroup"))] : []),
        ],
      });
    }
  }
  return (
    <>
      <span
        tabIndex={-1}
        onPointerDown={(event) => {
          if (event.button === 0) event.currentTarget.focus({ preventScroll: true });
        }}
        className="workbench-tab-label"
        title={title}
        data-panel-instance-id={props.panelInstanceId}
        onContextMenu={(event) => {
          event.preventDefault();
          event.stopPropagation();
          if (!(metadata.role === "view" && isWorkbenchActivityViewId(metadata.viewId)))
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
