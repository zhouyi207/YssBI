import {
  forwardRef,
  memo,
  useCallback,
  useEffect,
  useMemo,
  useState,
  useSyncExternalStore,
  type FunctionComponent,
  type ReactNode,
} from "react";
import {
  Actions,
  BorderNode,
  DockLocation,
  GroupAction,
  Layout,
  Model,
  TabNode,
  TabSetNode,
  type Action,
} from "flexlayout-react";
import { useTranslation } from "react-i18next";
import { VscChromeClose, VscLinkExternal, VscLayoutPanelCenter } from "react-icons/vsc";
import { workbenchLayoutControl } from "./workbenchControl";
import { showWorkbenchLayoutError } from "../application/workbenchLayoutErrorFeedback";
import {
  DndContext,
  DragOverlay,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
  type DragStartEvent,
} from "@dnd-kit/core";
import { useWorkbenchLayout } from "../application/useWorkbenchLayout";
import { workbenchLayoutController } from "../application/workbenchLayoutController";
import { snapTopLeftToCursor } from "../ui/dnd/snapTopLeftToCursorModifier";
import { WorkbenchSettingsButton } from "../ui/status/StatusBar";
import { workbenchLayoutRead } from "./workbenchRead";
import { workbenchLayoutRootBinding } from "./workbenchRootBinding";
import type {
  RootPanelActivationTarget,
  RootPanelRegistry,
  RootPanelTabComponent,
  RootPanelProps,
} from "./panelContribution";

export interface RootLayoutDndCoordinator {
  readonly onDragStart: (event: DragStartEvent) => void;
  readonly onDragEnd: (event: DragEndEvent) => void;
}
export type RootPanelActivationCoordinator = (panel: RootPanelActivationTarget) => void;
export interface RootLayoutHostProps {
  readonly panelRegistry: RootPanelRegistry;
  readonly tabComponent: RootPanelTabComponent;
  readonly dndCoordinator: RootLayoutDndCoordinator;
  readonly onActiveEditorPanelChange: RootPanelActivationCoordinator;
  readonly onClosePanels: (panelInstanceIds: readonly string[]) => void;
  readonly layoutTheme: string;
  readonly watermarkComponent: FunctionComponent;
  readonly statusBar: ReactNode;
  readonly dragOverlay?: ReactNode;
}
function panelProps(node: TabNode): RootPanelProps | undefined {
  const panel = workbenchLayoutRead.getPanel(node.getId());
  return panel
    ? {
        panelInstanceId: panel.panelInstanceId,
        groupId: panel.groupId,
        params: { metadata: panel.metadata },
        title: panel.title ?? node.getName(),
        visible: panel.visible ?? false,
      }
    : undefined;
}
const PanelContent = memo(function PanelContent({
  id,
  registry,
}: {
  id: string;
  registry: RootPanelRegistry;
}) {
  const selectPanel = useMemo(() => {
    let previous: RootPanelProps | undefined;
    return () => {
      const panel = workbenchLayoutRead.getPanel(id);
      if (!panel) {
        previous = undefined;
        return undefined;
      }
      if (
        previous &&
        previous.groupId === panel.groupId &&
        previous.title === (panel.title ?? "") &&
        previous.visible === (panel.visible ?? false) &&
        previous.params.metadata === panel.metadata
      )
        return previous;
      previous = {
        panelInstanceId: id,
        groupId: panel.groupId,
        params: { metadata: panel.metadata },
        title: panel.title ?? "",
        visible: panel.visible ?? false,
      };
      return previous;
    };
  }, [id]);
  const subscribePanel = useCallback(
    (listener: () => void) => workbenchLayoutRead.subscribePanel(id, listener),
    [id],
  );
  const props = useSyncExternalStore(subscribePanel, selectPanel, selectPanel);
  if (!props) return null;
  const component = workbenchLayoutRead.getPanel(id)?.component;
  if (!component) return null;
  const Component = registry[component];
  return (
    <div
      className="h-full min-h-0 w-full min-w-0 overflow-hidden"
      data-panel-instance-id={id}
      tabIndex={-1}
      onPointerDownCapture={(event) => {
        if (!(event.target instanceof Element) || !event.currentTarget.contains(event.target))
          return;
        workbenchLayoutRootBinding.activatePanel(id);
        const control = event.target.closest(
          "input, textarea, select, button, a[href], [contenteditable], [tabindex]",
        );
        // Give non-native canvas surfaces a DOM owner for subsequent keyboard events too.
        (control instanceof HTMLElement ? control : event.currentTarget).focus({
          preventScroll: true,
        });
      }}
      onFocusCapture={(event) => {
        if (event.currentTarget.contains(event.target))
          workbenchLayoutRootBinding.activatePanel(id);
      }}
    >
      <Component {...props} />
    </div>
  );
});
export const RootLayoutHost = memo(
  forwardRef<HTMLDivElement, RootLayoutHostProps>(
    (
      {
        panelRegistry,
        tabComponent: TabComponent,
        dndCoordinator,
        onActiveEditorPanelChange,
        onClosePanels,
        layoutTheme,
        watermarkComponent: Watermark,
        statusBar,
        dragOverlay,
      },
      ref,
    ) => {
      const { t } = useTranslation();
      const [binding] = useState(workbenchLayoutRootBinding.create);
      const model = useSyncExternalStore(
        binding.subscribeModel,
        binding.getModel,
        binding.getModel,
      );
      const sensors = useSensors(
        useSensor(PointerSensor, { activationConstraint: { distance: 5 } }),
      );
      useWorkbenchLayout(binding);
      useEffect(() => {
        let lastTarget: string | undefined;
        return workbenchLayoutRead.subscribeActivePanel(() => {
          if (!workbenchLayoutRead.isHydrated || !workbenchLayoutController.projectResourcesReady)
            return;
          const panel = workbenchLayoutRead.getActivePanel();
          if (panel?.metadata.role !== "editor") {
            lastTarget = undefined;
            return;
          }
          const key = JSON.stringify([
            panel.panelInstanceId,
            panel.groupId,
            panel.metadata.resourceRef,
          ]);
          if (lastTarget === key) return;
          lastTarget = key;
          onActiveEditorPanelChange({ ...panel, metadata: panel.metadata });
        });
      }, [onActiveEditorPanelChange]);
      const factory = useCallback(
        (node: TabNode) => <PanelContent id={node.getId()} registry={panelRegistry} />,
        [panelRegistry],
      );
      const closingPanels = (action: Action): string[] => {
        if (action instanceof GroupAction) return action.actions.flatMap(closingPanels);
        if (action.type === Actions.DELETE_TAB) return [action.data.node];
        if (action.type === Actions.DELETE_TABSET)
          return workbenchLayoutRead
            .listGroupPanels(action.data.node)
            .map((panel) => panel.panelInstanceId);
        return [];
      };
      return (
        <DndContext
          sensors={sensors}
          onDragStart={dndCoordinator.onDragStart}
          onDragEnd={dndCoordinator.onDragEnd}
        >
          <div
            ref={ref}
            className={"workbench-layout-host relative min-h-0 min-w-0 flex-1 " + layoutTheme}
            data-yssbi-root-layout
            data-testid="root-layout"
          >
            <Layout
              model={model}
              factory={factory}
              supportsPopout={false}
              constrainFloatPanels
              realtimeResize
              invalidateTabContentOnParentRender={false}
              onAction={(action) => {
                const ids = closingPanels(action);
                if (ids.length) onClosePanels(ids);
                else workbenchLayoutRootBinding.dispatchAction(action);
                return undefined;
              }}
              onAuxMouseClick={(node, event) => {
                if (event.button === 1 && node instanceof TabNode) {
                  event.preventDefault();
                  onClosePanels([node.getId()]);
                }
              }}
              onRenderTab={(node, values) => {
                const props = panelProps(node);
                if (props) values.content = <TabComponent {...props} />;
              }}
              onRenderTabSet={(node, values) => {
                const layoutId = node.getLayoutId();
                const floatToolbarGroup =
                  layoutId !== Model.MAIN_LAYOUT_ID
                    ? (model.getMaximizedTabset(layoutId) ??
                      model.getFirstTabSet(model.getRootRow(layoutId)))
                    : undefined;
                if (node instanceof TabSetNode && node === floatToolbarGroup) {
                  values.buttons.push(
                    <button
                      key="dock-float"
                      type="button"
                      className="flexlayout__tab_toolbar_button"
                      title={t("tabBar.contextMenu.dockFloat")}
                      aria-label={t("tabBar.contextMenu.dockFloat")}
                      onPointerDown={(event) => event.stopPropagation()}
                      onClick={() =>
                        void workbenchLayoutControl
                          .dockFloat(layoutId)
                          .catch(showWorkbenchLayoutError)
                      }
                    >
                      <VscLayoutPanelCenter aria-hidden />
                    </button>,
                    <button
                      key="close-float"
                      type="button"
                      className="flexlayout__tab_toolbar_button"
                      title={t("tabBar.contextMenu.closeFloat")}
                      aria-label={t("tabBar.contextMenu.closeFloat")}
                      onPointerDown={(event) => event.stopPropagation()}
                      onClick={() =>
                        onClosePanels(
                          workbenchLayoutRead
                            .listPanels()
                            .filter(
                              (panel) =>
                                panel.location.type === "float" &&
                                panel.location.layoutId === layoutId,
                            )
                            .map((panel) => panel.panelInstanceId),
                        )
                      }
                    >
                      <VscChromeClose aria-hidden />
                    </button>,
                  );
                }
                if (
                  node instanceof TabSetNode &&
                  node.getLayoutId() === Model.MAIN_LAYOUT_ID &&
                  node.getTabNodes().length > 1 &&
                  node.getTabNodes().every((tab) => tab.isEnableFloat())
                ) {
                  values.buttons.push(
                    <button
                      key="float-group"
                      type="button"
                      className="flexlayout__tab_toolbar_button"
                      title={t("tabBar.contextMenu.floatGroup")}
                      aria-label={t("tabBar.contextMenu.floatGroup")}
                      onPointerDown={(event) => event.stopPropagation()}
                      onClick={() =>
                        void workbenchLayoutControl
                          .floatGroup(node.getId())
                          .catch(showWorkbenchLayoutError)
                      }
                    >
                      <VscLinkExternal aria-hidden />
                    </button>,
                  );
                }
                if (!(node instanceof BorderNode)) return;
                if (node.getLocation() === DockLocation.BOTTOM) {
                  values.leading = <WorkbenchSettingsButton />;
                  values.buttons.push(
                    <div key="workbench-status" className="min-w-0">
                      {statusBar}
                    </div>,
                  );
                }
              }}
              onTabSetPlaceHolder={() =>
                workbenchLayoutRead
                  .listGroups()
                  .some((group) => group.location.type === "grid") ? null : (
                  <div className="flexlayout__tab inset-0" data-workbench-watermark>
                    <Watermark />
                  </div>
                )
              }
            />
          </div>
          <DragOverlay dropAnimation={null} modifiers={[snapTopLeftToCursor]}>
            {dragOverlay}
          </DragOverlay>
        </DndContext>
      );
    },
  ),
);
RootLayoutHost.displayName = "RootLayoutHost";
