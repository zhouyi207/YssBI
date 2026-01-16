import React, { useRef } from "react";
import { Pin } from "../Pin";
import { MathNode, BaseNode } from "./models";

export interface NodeProps {
  node: BaseNode;
  scale: number;
  onDrag?: (id: string, dx: number, dy: number) => void;
  onAddInput?: (id: string) => void;
  onPinClick?: (pinId: string, direction: "input" | "output") => void;
}

/* ================= Default Node UI ================= */

const DefaultNodeUI: React.FC<NodeProps & { onPinClick?: any }> = ({
  node,
  onPinClick,
}) => (
  <>
    <div className="flex items-center justify-between gap-2 px-3 py-1.5 text-sm font-semibold bg-black/5 rounded-t border-b border-black/5">
      <div className="flex items-center gap-2">
        <span>{node.title}</span>
      </div>
      <div className="text-[10px] opacity-40 font-mono uppercase tracking-tighter">
        {node.category}
      </div>
    </div>
    <div className="flex gap-2 px-2 py-2 whitespace-nowrap min-h-[40px] items-center">
      <div className="flex flex-col gap-1 flex-1">
        {node.inputs.map((pin) => (
          <Pin key={pin.id} {...pin} onPinClick={onPinClick} />
        ))}
      </div>
      <div className="flex-1" />
      <div className="flex flex-col gap-1 flex-1 items-end">
        {node.outputs.map((pin) => (
          <Pin key={pin.id} {...pin} onPinClick={onPinClick} />
        ))}
      </div>
    </div>
  </>
);

/* ================= Math Node UI ================= */

const MathNodeUI: React.FC<NodeProps & { onPinClick?: any }> = ({
  node,
  onAddInput,
  onPinClick,
}) => (
  <div className="relative flex gap-4 px-2 py-2 min-h-full items-center">
    {node.centerSymbol && (
      <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
        <span className="text-2xl font-bold opacity-30 text-black/40">
          {node.centerSymbol}
        </span>
      </div>
    )}
    <div className="flex flex-col gap-1 z-10 items-start">
      {node.inputs.map((pin) => (
        <Pin key={pin.id} {...pin} onPinClick={onPinClick} />
      ))}
      <button
        onClick={(e) => {
          e.stopPropagation();
          onAddInput?.(node.id);
        }}
        onPointerDown={(e) => e.stopPropagation()}
        className="mt-1 w-4 h-4 flex items-center justify-center bg-black/10 hover:bg-black/20 rounded text-[10px]"
      >
        +
      </button>
    </div>
    <div className="flex-1" />
    <div className="flex flex-col gap-1 z-10 items-end">
      {node.outputs.map((pin) => (
        <Pin key={pin.id} {...pin} onPinClick={onPinClick} />
      ))}
    </div>
  </div>
);

/* ================= Main Dispatcher ================= */

export const Node = React.memo<NodeProps>((props) => {
  const { node, scale, onDrag } = props;
  const dragging = useRef(false);
  const last = useRef({ x: 0, y: 0 });

  const onPointerDown = (e: React.PointerEvent) => {
    e.stopPropagation();
    e.preventDefault();
    dragging.current = true;
    last.current = { x: e.clientX, y: e.clientY };
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
  };

  const onPointerMove = (e: PointerEvent) => {
    if (!dragging.current) return;
    const dx = (e.clientX - last.current.x) / scale;
    const dy = (e.clientY - last.current.y) / scale;
    if (onDrag) onDrag(node.id, dx, dy);
    last.current = { x: e.clientX, y: e.clientY };
  };

  const onPointerUp = () => {
    dragging.current = false;
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
  };

  return (
    <div
      id={node.id}
      className={`absolute select-none rounded shadow-md border cursor-move ${
        node.selected
          ? "border-blue-500 ring-2 ring-blue-500/50"
          : "border-gray-300"
      }`}
      style={{
        minWidth: node.noHeader ? 120 : 160,
        minHeight: node.noHeader ? 60 : undefined,
        background: "#ffffff",
        transform: `translate(${node.position.x}px, ${node.position.y}px)`,
      }}
      onPointerDown={onPointerDown}
    >
      {node instanceof MathNode ? (
        <MathNodeUI {...props} />
      ) : (
        <DefaultNodeUI {...props} />
      )}
    </div>
  );
}, (prev, next) => {
  // 性能优化核心：只有以下属性变化时才重绘节点
  return (
    prev.node.id === next.node.id &&
    prev.node.position.x === next.node.position.x &&
    prev.node.position.y === next.node.position.y &&
    prev.node.selected === next.node.selected &&
    prev.scale === next.scale &&
    prev.node.inputs.length === next.node.inputs.length &&
    prev.node.outputs.length === next.node.outputs.length
  );
});
