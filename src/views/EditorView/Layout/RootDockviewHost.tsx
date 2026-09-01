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

import { executeEditorDragEnd } from "@/features/application/editor/editorDragDropActions";
import { synchronizeActiveEditorPanel } from "@/features/application/editor/activateEditorPanelAndSyncSession";
import { useWorkbenchLayout } from "@/features/application/layout/useWorkbenchLayout";
import { workbenchLayoutController } from "@/features/application/layout/workbenchLayoutController";
import { workbenchDockviewRead } from "@/features/core/dockview";
import type { WorkbenchComponentId, WorkbenchPanelParams } from "@/features/core/dockview";
import {
  buildSidebarDragState,
  isSidebarSpawnDrag,
  parseCanvasDragPayload,
} from "@/features/core/dnd";
import { snapTopLeftToCursor } from "@/features/core/dnd/snapTopLeftToCursorModifier";
import { keyboardUi } from "@/features/core/keyboard/ui";
import { useSettingsRead } from "@/features/core/settings/read";
import { sidebarDragUi } from "@/features/core/sidebarDrag/ui";
import { resolveYssbiDockviewTheme } from "@/shared/theme/dockviewTheme";
import { addGlobalEventListener } from "@/shared/utils/globalEvent";
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
  { readonly panelRegistry: RootPanelRegistry }
>(({ panelRegistry }, ref) => {
  const setActiveDrag = sidebarDragUi.setActiveDrag;
  const updatePosition = sidebarDragUi.updatePosition;
  const setModifierKeys = keyboardUi.setModifierKeys;
  const themeMode = useSettingsRead((state) => state.theme.mode);
  const bindWorkbenchLayout = useWorkbenchLayout();
  const activationDisposableRef = useRef<{ dispose(): void } | null>(null);
  const pointerMoveCleanupRef = useRef<(() => void) | null>(null);
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 5 } }));

  useEffect(
    () => () => {
      activationDisposableRef.current?.dispose();
      activationDisposableRef.current = null;
      pointerMoveCleanupRef.current?.();
      pointerMoveCleanupRef.current = null;
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

  const finishSidebarDrag = useCallback(() => {
    pointerMoveCleanupRef.current?.();
    pointerMoveCleanupRef.current = null;
    setActiveDrag(null);
  }, [setActiveDrag]);

  const handleDragStart = (event: DragStartEvent) => {
    const activeData = parseCanvasDragPayload(event.active.data.current);
    if (!isSidebarSpawnDrag(activeData)) return;
    const activatorEvent = event.activatorEvent as PointerEvent;
    setActiveDrag(
      buildSidebarDragState(activeData, activatorEvent?.clientX ?? 0, activatorEvent?.clientY ?? 0),
    );
    pointerMoveCleanupRef.current?.();
    pointerMoveCleanupRef.current = addGlobalEventListener(
      document,
      "pointermove",
      (pointerEvent) => {
        updatePosition(pointerEvent.clientX, pointerEvent.clientY);
        setModifierKeys(pointerEvent);
      },
    );
  };

  const handleDragEnd = (event: DragEndEvent) => {
    void executeEditorDragEnd(event, { finishSidebarDrag });
  };

  return (
    <DndContext sensors={sensors} onDragStart={handleDragStart} onDragEnd={handleDragEnd}>
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
