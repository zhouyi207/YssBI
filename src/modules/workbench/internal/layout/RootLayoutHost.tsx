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
import { Actions, BorderNode, DockLocation, Layout, TabNode } from "flexlayout-react";
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
export type RootPanelActivationCoordinator = (
  panel: RootPanelActivationTarget,
) => void | Promise<void>;
export interface RootLayoutHostProps {
  readonly panelRegistry: RootPanelRegistry;
  readonly tabComponent: RootPanelTabComponent;
  readonly dndCoordinator: RootLayoutDndCoordinator;
  readonly onActiveEditorPanelChange: RootPanelActivationCoordinator;
  readonly onClosePanel: (panelInstanceId: string) => void;
  readonly onCloseGroup: (groupId: string) => void;
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
  const props = useSyncExternalStore(workbenchLayoutRead.subscribe, selectPanel, selectPanel);
  if (!props) return null;
  const component = workbenchLayoutRead.getPanel(id)?.component;
  if (!component) return null;
  const Component = registry[component];
  return (
    <div
      className="h-full min-h-0 w-full min-w-0 overflow-hidden"
      data-panel-instance-id={id}
      onPointerDownCapture={() => workbenchLayoutRootBinding.focusPanel(id)}
      onFocusCapture={() => workbenchLayoutRootBinding.focusPanel(id)}
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
        onClosePanel,
        onCloseGroup,
        layoutTheme,
        watermarkComponent: Watermark,
        statusBar,
        dragOverlay,
      },
      ref,
    ) => {
      const [binding] = useState(workbenchLayoutRootBinding.create);
      const model = useSyncExternalStore(binding.subscribe, binding.getModel, binding.getModel);
      const sensors = useSensors(
        useSensor(PointerSensor, { activationConstraint: { distance: 5 } }),
      );
      useWorkbenchLayout(binding);
      useEffect(() => {
        let lastTarget: string | undefined;
        return workbenchLayoutRead.subscribe(() => {
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
          void onActiveEditorPanelChange({ ...panel, metadata: panel.metadata });
        });
      }, [onActiveEditorPanelChange]);
      const factory = useCallback(
        (node: TabNode) => <PanelContent id={node.getId()} registry={panelRegistry} />,
        [panelRegistry],
      );
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
              realtimeResize
              invalidateTabContentOnParentRender={false}
              onAction={(action) => {
                if (action.type === Actions.DELETE_TAB) onClosePanel(action.data.node);
                else if (action.type === Actions.DELETE_TABSET) onCloseGroup(action.data.node);
                else workbenchLayoutRootBinding.dispatchAction(action);
                return undefined;
              }}
              onAuxMouseClick={(node, event) => {
                if (event.button === 1 && node instanceof TabNode) {
                  event.preventDefault();
                  onClosePanel(node.getId());
                }
              }}
              onRenderTab={(node, values) => {
                const props = panelProps(node);
                if (props) values.content = <TabComponent {...props} />;
              }}
              onRenderTabSet={(node, values) => {
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
                  <div className="h-full min-h-0 w-full" data-workbench-watermark>
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
