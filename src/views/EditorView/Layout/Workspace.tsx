import { forwardRef, useState, useEffect, useRef } from "react";
import { formatErrorMessage } from "@/shared/utils/formatErrorMessage";
import { openGraphInEditor } from "@/features/application/editor/openGraphInEditor";
import { uiStore } from "@/features/core/ui/UIStore";
import { LayoutNodeRenderer } from "../Renderer/LayoutNodeRenderer";
import { DndContext, useSensor, useSensors, PointerSensor, DragEndEvent, DragOverEvent, DragStartEvent, DragOverlay } from '@dnd-kit/core';
import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { useSidebarDragStore, canvasDropHandlerStore } from "@/features/core/sidebarDrag";
import { useModifierKeyStore } from "@/features/core/keyboard";
import {
  DRAG_TYPES,
  isCanvasDrop,
  isLayoutRegionDrop,
  isTabbarDrop,
  type GraphResourceDragData,
} from "@/features/core/dnd";
import { addGlobalEventListener } from "@/shared/utils/globalEvent";
import { DropIndicator } from "../Renderer/DropIndicator";
import { SidebarDragOverlay } from "./SidebarDragOverlay";
import { LayoutNode } from "@/shared/types/ui";
import "../Renderer/viewRegistry"; // 确保业务组件已注册

export const Workspace = forwardRef<HTMLDivElement, { nodeId: string }>(({ nodeId }, ref) => {
  const moveNode = useLayoutStore(s => s.moveNode);
  const moveTab = useLayoutStore(s => s.moveTab);
  const setDragging = useLayoutStore(s => s.setDragging);
  const setActiveDrag = useSidebarDragStore(s => s.setActiveDrag);
  const updatePosition = useSidebarDragStore(s => s.updatePosition);

  const pointerMoveCleanupRef = useRef<(() => void) | null>(null);

  const [dropState, setDropState] = useState<{ visible: boolean; position: any; type?: 'dock' | 'merge' }>({
    visible: false, position: {}, type: 'dock'
  });

  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: {
        distance: 5,
      },
    })
  );

  useEffect(() => {
    return () => {
      pointerMoveCleanupRef.current?.();
      pointerMoveCleanupRef.current = null;
    };
  }, []);

  const finishSidebarDrag = () => {
    pointerMoveCleanupRef.current?.();
    pointerMoveCleanupRef.current = null;
    setActiveDrag(null);
  };

  const handleDragStart = (event: DragStartEvent) => {
    setDragging(true);
    const activeData = event.active.data.current as any;
    if (activeData?.type === DRAG_TYPES.NODE_TEMPLATE || activeData?.type === DRAG_TYPES.GRAPH_RESOURCE) {
      const activatorEvent = event.activatorEvent as PointerEvent;
      const x = activatorEvent?.clientX ?? 0;
      const y = activatorEvent?.clientY ?? 0;
      const title =
        activeData.sidebarResource?.name ??
        activeData.template?.title ??
        activeData.template?.variableName ??
        activeData.template?.subName ??
        activeData.template?.nodeType;
      setActiveDrag({
        type: activeData.type,
        template: { ...activeData.template, title },
        sidebarResource: activeData.sidebarResource,
        x,
        y,
      });
      const onMove = (e: PointerEvent) => updatePosition(e.clientX, e.clientY);
      pointerMoveCleanupRef.current?.();
      pointerMoveCleanupRef.current = addGlobalEventListener(document, "pointermove", onMove);
    }
  };

  const handleDragOver = (event: DragOverEvent) => {
    const { over, active } = event;
    const activeData = active.data.current as any;

    // Sidebar drags are handled by canvas drop zones, so hide layout docking preview.
    if (activeData?.type === DRAG_TYPES.NODE_TEMPLATE || activeData?.sidebarResource) {
      setDropState(s => ({ ...s, visible: false }));
      return;
    }

    if (!over) {
      setDropState(s => ({ ...s, visible: false }));
      return;
    }

    const overData = over.data.current;
    
    if (isTabbarDrop(overData)) {
      setDropState(s => ({ ...s, visible: false }));
      return;
    }

    if (!isLayoutRegionDrop(overData)) {
      setDropState(s => ({ ...s, visible: false }));
      return;
    }

    const targetNodeId = overData.targetNodeId;
    const dropPosition = overData.dropPosition;
    
    // 找到目标叶子节点的完整 DOM 元素
    const targetElement = document.getElementById(`layout-node-${targetNodeId}`);

    if (targetElement) {
      const rect = targetElement.getBoundingClientRect();
      let indicatorPos = {
        top: rect.top,
        left: rect.left,
        width: rect.width,
        height: rect.height
      };

      // 根据停靠位置计算预览区域（实现 50/50 预览，而非 20% 区域预览）
      // center 默认为右侧分屏（VSCode 行为）
      const actualPosition = dropPosition === 'center' ? 'right' : dropPosition;
      
      if (actualPosition === 'left') {
        indicatorPos.width /= 2;
      } else if (actualPosition === 'right') {
        indicatorPos.width /= 2;
        indicatorPos.left += indicatorPos.width;
      } else if (actualPosition === 'top') {
        indicatorPos.height /= 2;
      } else if (actualPosition === 'bottom') {
        indicatorPos.height /= 2;
        indicatorPos.top += indicatorPos.height;
      }

      setDropState({
        visible: true,
        position: indicatorPos,
        type: 'dock'
      });
    } else {
      setDropState(s => ({ ...s, visible: false }));
    }
  };

  const handleDragEnd = (event: DragEndEvent) => {
    setDragging(false);
    setDropState(s => ({ ...s, visible: false }));
    const { active, over } = event;
    const activeData = active.data.current as any;
    const overData = over?.data.current;
    const sidebarResource = activeData?.sidebarResource as
      | GraphResourceDragData
      | undefined;

    if (sidebarResource && isCanvasDrop(overData)) {
      finishSidebarDrag();

      const targetGroupId = overData.groupId || useLayoutStore.getState().activeEditorGroupId || "default_editor";
      void openGraphInEditor(sidebarResource.id, sidebarResource.name, sidebarResource.type, targetGroupId)
        .catch((error) => uiStore.showToast(formatErrorMessage(error), "error"));
      return;
    }

    if (activeData?.type === DRAG_TYPES.GRAPH_RESOURCE) {
      finishSidebarDrag();
      return;
    }

    if (activeData?.type === DRAG_TYPES.NODE_TEMPLATE) {
      const dragState = useSidebarDragStore.getState().activeDrag;
      finishSidebarDrag();
      const groupId = isCanvasDrop(overData) ? overData.groupId : null;
      if (groupId && dragState) {
        // 将目标 canvas 设为 active group（确保 variable drop menu 等 UI 正确显示）
        useLayoutStore.getState().setActiveGroup(groupId);

        const handler = canvasDropHandlerStore.getHandler(groupId);
        if (handler) {
          const modifierKeys = useModifierKeyStore.getState();
          handler(dragState, {
            altKey: modifierKeys.altKey || (event.activatorEvent as PointerEvent)?.altKey || false,
            ctrlKey: modifierKeys.ctrlKey || (event.activatorEvent as PointerEvent)?.ctrlKey || false,
          });
        }
      }
      return;
    }

    if (over && active.id !== over.id) {
      const dropData = over.data.current;
      const dropPosition = isLayoutRegionDrop(dropData) ? dropData.dropPosition : 'center';
      const targetNodeId = isLayoutRegionDrop(dropData) || isTabbarDrop(dropData)
        ? dropData.targetNodeId
        : over.id;

      if (activeData?.type === DRAG_TYPES.TAB) {
        // 处理 Tab 拖拽
        const sourceNodeId = activeData.sourceNodeId;
        const tabId = activeData.tabId;
        const isTabbarTarget = isTabbarDrop(dropData);

        if (isTabbarTarget) {
          // 拖到 TabBar：移动标签页（VSCode 行为 - 从源编辑器删除）
          const targetTabIndex = dropData.targetTabIndex;
          moveTab(sourceNodeId, tabId, targetNodeId as string, targetTabIndex);
        } else {
          // 拖到画布区域（center/top/bottom/left/right）：分屏（VSCode 行为 - 复制标签页）
          const layoutStore = useLayoutStore.getState();
          const sourceNode = layoutStore.nodes[sourceNodeId];
          const targetNode = layoutStore.nodes[targetNodeId as string];
          const tabToMove = sourceNode?.data?.tabs?.find(t => t.id === tabId);
          
          if (tabToMove && targetNode) {
            // center 默认为右侧分屏（VSCode 行为）
            const actualPosition = dropPosition === 'center' ? 'right' : dropPosition;
            
            // 根据拖放位置决定分屏方向和位置
            const direction: 'row' | 'col' = (actualPosition === 'left' || actualPosition === 'right') ? 'row' : 'col';
            const isAfter = actualPosition === 'right' || actualPosition === 'bottom';
            const newNodeId = Math.random().toString(36).slice(2, 11);
            
            useLayoutStore.setState((state) => {
              const targetNode = state.nodes[targetNodeId as string];
              if (!targetNode || !targetNode.parentId) return;
              
              const parentNode = state.nodes[targetNode.parentId];
              const requiredDirection = direction;
              
              // 创建新的编辑器组节点（复制标签页）
              const newNode: LayoutNode = {
                id: newNodeId,
                type: 'component',
                parentId: parentNode.id,
                children: [],
                size: 1,
                data: {
                  component: tabToMove.component || 'GraphEditor',
                  tabs: [{ ...tabToMove }],
                  activeTabId: tabId
                }
              };
              
              if (parentNode.type === requiredDirection) {
                // 父节点方向一致，直接插入
                const targetIndex = parentNode.children?.indexOf(targetNodeId as string) || 0;
                const insertIndex = isAfter ? targetIndex + 1 : targetIndex;
                parentNode.children?.splice(insertIndex, 0, newNodeId);
                state.nodes[newNodeId] = newNode;
              } else {
                // 需要创建新的分支容器
                const branchId = Math.random().toString(36).slice(2, 11);
                const branch: LayoutNode = {
                  id: branchId,
                  type: requiredDirection,
                  parentId: parentNode.id,
                  children: isAfter ? [targetNodeId as string, newNodeId] : [newNodeId, targetNodeId as string],
                  size: targetNode.size,
                  pixelSize: targetNode.pixelSize
                };
                
                const targetIndex = parentNode.children?.indexOf(targetNodeId as string) || 0;
                parentNode.children![targetIndex] = branchId;
                
                targetNode.parentId = branchId;
                targetNode.size = 1;
                targetNode.pixelSize = undefined;
                
                newNode.parentId = branchId;
                
                state.nodes[newNodeId] = newNode;
                state.nodes[branchId] = branch;
              }
              
              // 拖到画布区域时，总是保留源标签页（分屏复制，而不是移动）
              // 源编辑器保持不变
              
              // 激活新创建的编辑器组
              state.activeGroupId = newNodeId;
              state.activeEditorGroupId = newNodeId;
            });
          }
        }
      } else if (activeData?.type === DRAG_TYPES.LEAF) {
        // 处理节点拖拽
        moveNode(active.id as string, targetNodeId as string, dropPosition);
      }
    }
  };

  return (
    <DndContext
      sensors={sensors}
      onDragStart={handleDragStart}
      onDragEnd={handleDragEnd}
      onDragOver={handleDragOver}
    >
      <div ref={ref} className="flex-1 min-w-0 flex overflow-hidden relative">
        <div className="flex-1 min-w-0">
          <LayoutNodeRenderer nodeId={nodeId} />
        </div>

        {/* Local Drop Indicator for Workspace */}
        <DropIndicator {...dropState} />

        {/* Drag overlay for sidebar items */}
        <DragOverlay dropAnimation={null}>
          <SidebarDragOverlay />
        </DragOverlay>
      </div>
    </DndContext>
  );
});

Workspace.displayName = 'Workspace';
