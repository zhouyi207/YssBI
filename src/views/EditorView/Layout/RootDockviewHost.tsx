import {
  forwardRef,
  useCallback,
  useEffect,
  useRef,
  type FunctionComponent,
  type KeyboardEvent,
} from "react";
import { DockviewReact, type DockviewReadyEvent, type IDockviewPanelProps } from "dockview-react";
import {
  DndContext,
  DragOverlay,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
  type DragStartEvent,
} from "@dnd-kit/core";

import { synchronizeActiveEditorPanel } from "@/features/application/editor/activateEditorPanelAndSyncSession";
import { useWorkbenchLayout } from "@/features/application/layout/useWorkbenchLayout";
import { workbenchLayoutController } from "@/features/application/layout/workbenchLayoutController";
import { workbenchDockviewRead } from "@/features/core/dockview";
import type { WorkbenchComponentId, WorkbenchPanelParams } from "@/features/core/dockview";
import { snapTopLeftToCursor } from "@/features/core/dnd/snapTopLeftToCursorModifier";
import { useSettingsRead } from "@/features/core/settings/read";
import { resolveYssbiDockviewTheme } from "@/shared/theme/dockviewTheme";
import { WatermarkView } from "../Canvas/overlays/WatermarkView";
import { WorkbenchActivityActions } from "./WorkbenchActivityActions";
import { WorkbenchDockviewTab } from "./WorkbenchDockviewTab";
import { RootDockviewDragOverlay } from "./RootDockviewDragOverlay";

function WorkbenchDockviewWatermark() {
  return <WatermarkView />;
}

export type RootDockviewPanelComponent = FunctionComponent<
  IDockviewPanelProps<WorkbenchPanelParams>
>;

export type RootPanelRegistry = Record<WorkbenchComponentId, RootDockviewPanelComponent>;

export interface RootDockviewDndCoordinator {
  readonly onDragStart: (event: DragStartEvent) => void;
  readonly onDragEnd: (event: DragEndEvent) => void;
}

export function createRootPanelRegistry(registry: RootPanelRegistry): RootPanelRegistry {
  return registry;
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

export const RootDockviewHost = forwardRef<
  HTMLDivElement,
  {
    readonly panelRegistry: RootPanelRegistry;
    readonly dndCoordinator: RootDockviewDndCoordinator;
  }
>(({ panelRegistry, dndCoordinator }, ref) => {
  const themeMode = useSettingsRead((state) => state.theme.mode);
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
        void synchronizeActiveEditorPanel({ ...activePanel, metadata: activePanel.metadata });
      });
    },
    [bindWorkbenchLayout],
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
            defaultTabComponent={WorkbenchDockviewTab}
            rightHeaderActionsComponent={WorkbenchActivityActions}
            watermarkComponent={WorkbenchDockviewWatermark}
            disableFloatingGroups
            theme={resolveYssbiDockviewTheme(themeMode)}
            onReady={onDockviewReady}
          />
        </div>
      </div>
      <DragOverlay dropAnimation={null} modifiers={[snapTopLeftToCursor]}>
        <RootDockviewDragOverlay />
      </DragOverlay>
    </DndContext>
  );
});

RootDockviewHost.displayName = "RootDockviewHost";
