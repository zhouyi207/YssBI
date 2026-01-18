import React, { useRef, useEffect } from "react";
import { Pin } from "../Pin";
import { Pin as PinModel, MathNode, BaseNode } from "./models";

export interface NodeProps {
  node: BaseNode;
  scale: number;
  onDrag?: (id: string, dx: number, dy: number) => void;
  onAddInput?: (id: string) => void;
  onPinClick?: (pinId: string, direction: "input" | "output") => void;
  onPinPointerDown?: (e: React.PointerEvent, pin: PinModel) => void;
  onPointerDown?: (id: string, e: React.PointerEvent) => void;
}

/* ================= Default Node UI ================= */

const DefaultNodeUI: React.FC<NodeProps> = ({
  node,
  onPinClick,
  onPinPointerDown,
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
          <Pin
            key={pin.id}
            {...pin}
            onPinClick={onPinClick}
            onPinPointerDown={onPinPointerDown}
          />
        ))}
      </div>
      <div className="flex-1" />
      <div className="flex flex-col gap-1 flex-1 items-end">
        {node.outputs.map((pin) => (
          <Pin
            key={pin.id}
            {...pin}
            onPinClick={onPinClick}
            onPinPointerDown={onPinPointerDown}
          />
        ))}
      </div>
    </div>
  </>
);

/* ================= Math Node UI ================= */

const MathNodeUI: React.FC<NodeProps> = ({
  node,
  onAddInput,
  onPinClick,
  onPinPointerDown,
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
        <Pin
          key={pin.id}
          {...pin}
          onPinClick={onPinClick}
          onPinPointerDown={onPinPointerDown}
        />
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
        <Pin
          key={pin.id}
          {...pin}
          onPinClick={onPinClick}
          onPinPointerDown={onPinPointerDown}
        />
      ))}
    </div>
  </div>
);

/* ================= Main Dispatcher ================= */

export const Node = React.memo<NodeProps>((props) => {
  const { node } = props;
  const draggingRef = useRef(false);
  const last = useRef({ x: 0, y: 0 });
  const propsRef = useRef(props);

  // 始终保持最新的 props 引用，供事件监听器使用
  useEffect(() => {
    propsRef.current = props;
  });

  const onPointerDown = (e: React.PointerEvent) => {
    const { onPointerDown: currentOnPointerDown, node: currentNode } = propsRef.current;
    if (currentOnPointerDown) {
      currentOnPointerDown(currentNode.id, e);
    } else {
      e.stopPropagation();
      e.preventDefault();
    }

    draggingRef.current = true;
    last.current = { x: e.clientX, y: e.clientY };
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
  };

  const onPointerMove = (e: PointerEvent) => {
    if (!draggingRef.current) return;
    const { scale: currentScale, onDrag: currentOnDrag, node: currentNode } = propsRef.current;
    
    const dx = (e.clientX - last.current.x) / currentScale;
    const dy = (e.clientY - last.current.y) / currentScale;
    
    if (currentOnDrag) {
      currentOnDrag(currentNode.id, dx, dy);
    }
    last.current = { x: e.clientX, y: e.clientY };
  };

  const onPointerUp = () => {
    draggingRef.current = false;
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
  };

  // 组件卸载时清理监听器
  useEffect(() => {
    return () => {
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", onPointerUp);
    };
  }, []);

  return (
    <div
      id={node.id}
      className={`absolute select-none rounded shadow-md border cursor-move ${
        node.selected
          ? "border-blue-500 ring-2 ring-blue-500/50 z-30"
          : "border-gray-300 z-10"
      }`}
      style={{
        minWidth: node.noHeader ? 120 : 160,
        minHeight: node.noHeader ? 60 : undefined,
        transform: `translate(${node.position.x}px, ${node.position.y}px)`,
        background: "rgba(255, 255, 255, 0.6)",
        backdropFilter: "blur(4px)",
        WebkitBackdropFilter: "blur(4px)",
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
    prev.node.outputs.length === next.node.outputs.length &&
    // 检查连接状态 (引用比较，配合 clone 逻辑)
    prev.node.inputs.every((p, i) => p.links === next.node.inputs[i].links) &&
    prev.node.outputs.every((p, i) => p.links === next.node.outputs[i].links)
  );
});
