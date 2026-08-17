import { forwardRef, useCallback, useEffect, useRef, useState } from 'react';
import {
  DockviewDefaultTab,
  DockviewReact,
  GridviewReact,
  type DockviewApi,
  type DockviewReadyEvent,
  type GridviewApi,
  type GridviewReadyEvent,
  type IDockviewPanelHeaderProps,
  type IDockviewPanelProps,
  type IWatermarkPanelProps,
  Orientation,
} from 'dockview-react';
import {
  DndContext,
  DragOverlay,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
  type DragStartEvent,
} from '@dnd-kit/core';

import { closeEditorTab } from '@/features/application/editor/closeEditorTab';
import { synchronizeActiveEditorTab } from '@/features/application/editor/switchEditorTab';
import { executeEditorDragEnd } from '@/features/application/editor/editorDragDropActions';
import { editorDockviewPort, type DockviewPanelParams } from '@/features/core/dockview';
import { snapTopLeftToCursor, buildSidebarDragState, isSidebarSpawnDrag, parseCanvasDragPayload } from '@/features/core/dnd';
import { GroupContext } from '@/features/core/editor';
import { useModifierKeyStore } from '@/features/core/keyboard';
import { useSidebarDragStore } from '@/features/core/sidebarDrag';
import {
  DEFAULT_WORKBENCH_DETAIL_WIDTH,
  DEFAULT_WORKBENCH_SIDEBAR_WIDTH,
  WORKBENCH_DETAIL_PART_ID,
  WORKBENCH_EDITOR_PART_ID,
  WORKBENCH_SIDEBAR_PART_ID,
  useWorkbenchStore,
  workbenchGridPort,
} from '@/features/core/workbench';

import { addGlobalEventListener } from '@/shared/utils/globalEvent';
import type { LayoutTab } from '@/shared/types';
import { Detail } from './Detail/Detail';
import { PanelPart } from './PanelPart';
import Sidebar from './Sidebar';
import { WorkspaceDragOverlay } from './WorkspaceDragOverlay';
import { WatermarkView } from '../Canvas/overlays/WatermarkView';
import { viewRegistry } from '../Renderer/viewRegistry';


function useDockviewPanelGroupId(api: IDockviewPanelProps<DockviewPanelParams>['api']): string {
  const [groupId, setGroupId] = useState(() => api.group.id);

  useEffect(() => {
    const updateGroupId = () => setGroupId(api.group.id);
    const disposable = api.onDidGroupChange(updateGroupId);
    updateGroupId();
    return () => disposable.dispose();
  }, [api]);

  return groupId;
}

export function DockviewEditorPanel(props: IDockviewPanelProps<DockviewPanelParams>) {
  const Component = viewRegistry.get(props.api.component);
  const groupId = useDockviewPanelGroupId(props.api);

  return (
    <GroupContext.Provider value={groupId}>
      <div className="h-full w-full min-h-0 min-w-0 overflow-hidden bg-[var(--workbench-bg)]">
        {Component ? <Component /> : <div className="p-4 text-muted-foreground">No content</div>}
      </div>
    </GroupContext.Provider>
  );
}

export function DockviewEditorWatermark(_: IWatermarkPanelProps) {
  return <WatermarkView />;
}

function DockviewEditorTab(props: IDockviewPanelHeaderProps<DockviewPanelParams>) {
  const requestClose = useCallback(() => {
    const tab = props.params.layoutTab;
    void closeEditorTab(tab.resourceRef, props.api.group.id);
  }, [props.api, props.params.layoutTab]);

  return <DockviewDefaultTab {...props} closeActionOverride={requestClose} />;
}

function EditorDock() {
  const dockviewApiRef = useRef<DockviewApi | null>(null);
  const activationDisposableRef = useRef<{ dispose(): void } | null>(null);
  const onReady = useCallback((event: DockviewReadyEvent) => {
    dockviewApiRef.current = event.api;
    editorDockviewPort.bind(event.api);
    activationDisposableRef.current?.dispose();
    activationDisposableRef.current = event.api.onDidActivePanelChange(() => {
      const active = editorDockviewPort.getActivePanel();
      const value = active?.tab?.data?.layoutTab;
      if (active && value && typeof value === 'object') {
        void synchronizeActiveEditorTab(active.groupId, value as LayoutTab);
      }
    });
  }, []);

  useEffect(() => () => {
    activationDisposableRef.current?.dispose();
    const boundApi = dockviewApiRef.current;
    dockviewApiRef.current = null;
    if (boundApi) editorDockviewPort.unbind(boundApi);
  }, []);

  return (
    <DockviewReact
      className="dockview-theme-dark h-full w-full"
      components={{ GraphEditor: DockviewEditorPanel, WorksheetEditor: DockviewEditorPanel }}
      defaultTabComponent={DockviewEditorTab}
      watermarkComponent={DockviewEditorWatermark}
      disableFloatingGroups
      onReady={onReady}
    />
  );
}

function WorkbenchCenter() {
  return <PanelPart editorComponent={EditorDock} />;
}

const workbenchComponents = {
  sidebar: Sidebar,
  editor: WorkbenchCenter,
  detail: Detail,
};

function initializeWorkbench(api: GridviewApi): void {
  const editor = api.addPanel({ id: WORKBENCH_EDITOR_PART_ID, component: 'editor', minimumWidth: 240, minimumHeight: 120 });
  const sidebar = api.addPanel({
    id: WORKBENCH_SIDEBAR_PART_ID,
    component: 'sidebar',
    minimumWidth: 240,
    position: { direction: 'left', referencePanel: editor.id },
  });
  const detail = api.addPanel({
    id: WORKBENCH_DETAIL_PART_ID,
    component: 'detail',
    minimumWidth: 240,
    position: { direction: 'right', referencePanel: editor.id },
  });
  sidebar.api.setSize({ width: DEFAULT_WORKBENCH_SIDEBAR_WIDTH });
  detail.api.setSize({ width: DEFAULT_WORKBENCH_DETAIL_WIDTH });
}

function WorkbenchGrid() {
  const [api, setApi] = useState<GridviewApi | null>(null);
  const gridviewApiRef = useRef<GridviewApi | null>(null);
  const sidebarHidden = useWorkbenchStore((state) => state.sidebarUserHidden || state.zenMode);
  const detailHidden = useWorkbenchStore((state) => state.detailUserHidden || state.zenMode);

  const onReady = useCallback((event: GridviewReadyEvent) => {
    gridviewApiRef.current = event.api;
    initializeWorkbench(event.api);
    workbenchGridPort.bind(event.api);
    setApi(event.api);
  }, []);

  useEffect(() => () => {
    const boundApi = gridviewApiRef.current;
    gridviewApiRef.current = null;
    if (boundApi) workbenchGridPort.unbind(boundApi);
  }, []);

  useEffect(() => {
    if (!api) return;
    const visibility = [
      [WORKBENCH_SIDEBAR_PART_ID, !sidebarHidden],
      [WORKBENCH_DETAIL_PART_ID, !detailHidden],
    ] as const;
    for (const [id, visible] of visibility) {
      const panel = api.getPanel(id);
      panel?.api.setVisible(visible);
    }
  }, [api, detailHidden, sidebarHidden]);

  return (
    <GridviewReact
      className="h-full w-full"
      components={workbenchComponents}
      orientation={Orientation.HORIZONTAL}
      proportionalLayout={false}
      onReady={onReady}
    />
  );
}

export const Workspace = forwardRef<HTMLDivElement, { nodeId?: string }>((_, ref) => {
  const setActiveDrag = useSidebarDragStore((state) => state.setActiveDrag);
  const updatePosition = useSidebarDragStore((state) => state.updatePosition);
  const pointerMoveCleanupRef = useRef<(() => void) | null>(null);
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 5 } }));

  useEffect(() => () => pointerMoveCleanupRef.current?.(), []);

  const finishSidebarDrag = useCallback(() => {
    pointerMoveCleanupRef.current?.();
    pointerMoveCleanupRef.current = null;
    setActiveDrag(null);
  }, [setActiveDrag]);

  const handleDragStart = (event: DragStartEvent) => {
    const activeData = parseCanvasDragPayload(event.active.data.current);
    if (!isSidebarSpawnDrag(activeData)) return;
    const activatorEvent = event.activatorEvent as PointerEvent;
    setActiveDrag(buildSidebarDragState(activeData, activatorEvent?.clientX ?? 0, activatorEvent?.clientY ?? 0));
    pointerMoveCleanupRef.current?.();
    pointerMoveCleanupRef.current = addGlobalEventListener(document, 'pointermove', (pointerEvent) => {
      updatePosition(pointerEvent.clientX, pointerEvent.clientY);
      useModifierKeyStore.getState().setModifierKeys(pointerEvent);
    });
  };

  const handleDragEnd = (event: DragEndEvent) => {
    void executeEditorDragEnd(event, { finishSidebarDrag });
  };

  return (
    <DndContext sensors={sensors} onDragStart={handleDragStart} onDragEnd={handleDragEnd}>
      <div ref={ref} className="relative flex min-w-0 flex-1 overflow-hidden">
        <WorkbenchGrid />
      </div>
      <DragOverlay dropAnimation={null} modifiers={[snapTopLeftToCursor]}>
        <WorkspaceDragOverlay />
      </DragOverlay>
    </DndContext>
  );
});

Workspace.displayName = 'Workspace';
