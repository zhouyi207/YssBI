// Context/DragProvider.tsx
import React, { createContext, useContext, useState } from "react";
import {
  DndContext,
  useSensor,
  useSensors,
  PointerSensor,
  DragEndEvent,
  DragStartEvent,
} from "@dnd-kit/core";

interface DragContextValue {
  activeDrag: any | null;
}

const DragContext = createContext<DragContextValue | null>(null);

export function useDragContext() {
  const ctx = useContext(DragContext);
  if (!ctx) throw new Error("useDragContext must be used inside DragProvider");
  return ctx;
}

export const DragProvider: React.FC<{ children: React.ReactNode }> = ({
  children,
}) => {
  const [activeDrag, setActiveDrag] = useState<any>(null);

  const sensors = useSensors(useSensor(PointerSensor));

  const handleDragStart = (event: DragStartEvent) => {
    setActiveDrag(event.active.data.current); // 我们可以在 Draggable 元素里设置 data.current
  };

  const handleDragEnd = (event: DragEndEvent) => {
    setActiveDrag(null);
  };

  return (
    <DragContext.Provider value={{ activeDrag }}>
      <DndContext
        sensors={sensors}
        onDragStart={handleDragStart}
        onDragEnd={handleDragEnd}
      >
        {children}
      </DndContext>
    </DragContext.Provider>
  );
};
