import { forwardRef, useState } from "react";
import { LayoutNodeRenderer } from "../Renderer/LayoutNodeRenderer";
import { DndContext, useSensor, useSensors, PointerSensor, DragEndEvent, DragOverEvent, DragStartEvent } from '@dnd-kit/core';
import { useLayoutStore } from "../../../store/layoutStore";
import { DropIndicator } from "../Renderer/DropIndicator";
import "../Renderer/viewRegistry"; // 确保业务组件已注册


export const Workspace = forwardRef<HTMLDivElement, { nodeId: string }>(({ nodeId }, ref) => {
  const moveNode = useLayoutStore(s => s.moveNode);
  const moveTab = useLayoutStore(s => s.moveTab);
  const setDragging = useLayoutStore(s => s.setDragging);

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

  const handleDragStart = (_event: DragStartEvent) => {
    setDragging(true);
  };

  const handleDragOver = (event: DragOverEvent) => {
    const { active, over } = event;
    if (!over) {
      setDropState(s => ({ ...s, visible: false }));
      return;
    }

    const overId = over.id as string;
    const element = document.getElementById(overId) || document.getElementById(`layout-node-${overId}`);

    if (element && active.id !== overId) {
      const rect = element.getBoundingClientRect();
      const type = over.data.current?.dropPosition === 'center' ? 'merge' : 'dock';

      setDropState({
        visible: true,
        position: {
          top: rect.top,
          left: rect.left,
          width: rect.width,
          height: rect.height
        },
        type
      });
    }
  };

  const handleDragEnd = (event: DragEndEvent) => {
    setDragging(false);
    setDropState(s => ({ ...s, visible: false }));
    const { active, over } = event;

    if (over && active.id !== over.id) {
      const activeData = active.data.current as any;
      const dropData = over.data.current as any;
      const dropPosition = dropData?.dropPosition || 'center';
      const targetNodeId = dropData?.targetNodeId || over.id;

      if (activeData?.type === 'tab') {
        // 处理 Tab 拖拽
        moveTab(activeData.sourceNodeId, activeData.tabId, targetNodeId as string);
      } else if (activeData?.type === 'leaf') {
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
      </div>
    </DndContext>
  );
});

Workspace.displayName = 'Workspace';
