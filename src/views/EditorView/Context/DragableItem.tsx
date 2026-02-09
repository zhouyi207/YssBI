// Components/DraggableItem.tsx
import { useDraggable } from "@dnd-kit/core";

export const DraggableItem: React.FC<{ template: any }> = ({ template }) => {
  const { attributes, listeners, setNodeRef, isDragging } = useDraggable({
    id: template.id,
    data: { template }, // 这里存拖拽数据
  });

  return (
    <div
      ref={setNodeRef}
      {...listeners}
      {...attributes}
      className={`p-2 border rounded ${isDragging ? "opacity-50" : ""}`}
    >
      {template.title || template.type}
    </div>
  );
};
