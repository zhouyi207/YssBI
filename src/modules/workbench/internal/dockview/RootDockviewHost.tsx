import {
  forwardRef,
  useCallback,
  useEffect,
  useRef,
  type FunctionComponent,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import {
  DockviewReact,
  type DockviewReadyEvent,
  type DockviewTheme,
  type IDockviewHeaderActionsProps,
} from "dockview-react";
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
import { snapTopLeftToCursor } from "@/features/core/dnd/snapTopLeftToCursorModifier";
import { WorkbenchActivityActions } from "../ui/activity/WorkbenchActivityActions";
import { workbenchDockviewRead } from "./workbenchRead";
import type {
  RootPanelActivationTarget,
  RootPanelRegistry,
  RootPanelTabComponent,
} from "./panelContribution";

export interface RootDockviewDndCoordinator {
  readonly onDragStart: (event: DragStartEvent) => void;
  readonly onDragEnd: (event: DragEndEvent) => void;
}

export type RootPanelActivationCoordinator = (
  panel: RootPanelActivationTarget,
) => void | Promise<void>;

export interface RootDockviewHostProps {
  readonly panelRegistry: RootPanelRegistry;
  readonly tabComponent: RootPanelTabComponent;
  readonly dndCoordinator: RootDockviewDndCoordinator;
  readonly onActiveEditorPanelChange: RootPanelActivationCoordinator;
  readonly dockviewTheme: DockviewTheme;
  readonly watermarkComponent: FunctionComponent;
  readonly dragOverlay?: ReactNode;
  readonly activityActions?: ReactNode;
}

function preventDockviewNativeTabClose(event: KeyboardEvent<HTMLDivElement>): void {
  if (event.key !== "Delete" && event.key !== "Backspace") return;
  if (!(event.target instanceof Element)) return;
  const tab = event.target.closest(".dv-tab");
  const owningHost = tab?.closest("[data-yssbi-root-dockview], [data-yssbi-logs-dockview]");
  if (!tab || owningHost !== event.currentTarget) return;
  event.preventDefault();
  event.stopPropagation();
}

export const RootDockviewHost = forwardRef<HTMLDivElement, RootDockviewHostProps>(
  (
    {
      panelRegistry,
      tabComponent,
      dndCoordinator,
      onActiveEditorPanelChange,
      dockviewTheme,
      watermarkComponent,
      dragOverlay,
      activityActions,
    },
    ref,
  ) => {
    const bindWorkbenchLayout = useWorkbenchLayout();
    const activationDisposableRef = useRef<{ dispose(): void } | null>(null);
    const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 5 } }));

    useEffect(
      () => () => {
        activationDisposableRef.current?.dispose();
        activationDisposableRef.current = null;
      },
      [],
    );

    const onDockviewReady = useCallback(
      (event: DockviewReadyEvent) => {
        bindWorkbenchLayout(event);
        activationDisposableRef.current?.dispose();
        activationDisposableRef.current = event.api.onDidActivePanelChange(() => {
          if (!workbenchDockviewRead.isHydrated || !workbenchLayoutController.projectResourcesReady)
            return;

          const activePanel = workbenchDockviewRead.getActivePanel();
          if (activePanel?.metadata.role !== "editor") return;
          void onActiveEditorPanelChange({ ...activePanel, metadata: activePanel.metadata });
        });
      },
      [bindWorkbenchLayout, onActiveEditorPanelChange],
    );

    const rightHeaderActionsComponent = useCallback(
      (props: IDockviewHeaderActionsProps) => (
        <WorkbenchActivityActions {...props} additionalActions={activityActions} />
      ),
      [activityActions],
    );

    return (
      <DndContext
        sensors={sensors}
        onDragStart={dndCoordinator.onDragStart}
        onDragEnd={dndCoordinator.onDragEnd}
      >
        <div ref={ref} className="relative flex min-w-0 flex-1 overflow-hidden">
          <div
            data-yssbi-root-dockview
            data-testid="root-dockview"
            className="h-full min-h-0 w-full min-w-0"
            onKeyDownCapture={preventDockviewNativeTabClose}
          >
            <DockviewReact
              className="yssbi-root-dockview-instance h-full w-full"
              components={panelRegistry}
              defaultTabComponent={tabComponent}
              rightHeaderActionsComponent={rightHeaderActionsComponent}
              watermarkComponent={watermarkComponent}
              disableFloatingGroups
              theme={dockviewTheme}
              onReady={onDockviewReady}
            />
          </div>
        </div>
        <DragOverlay dropAnimation={null} modifiers={[snapTopLeftToCursor]}>
          {dragOverlay}
        </DragOverlay>
      </DndContext>
    );
  },
);

RootDockviewHost.displayName = "RootDockviewHost";
