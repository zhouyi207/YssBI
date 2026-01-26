import React from "react";
import { Pin } from "../Pins/Pin";
import { Pin as PinModel, BaseNode } from "../Types/nodes";
import { useSchemaStore } from "../Store/useSchemaStore";


export interface NodeProps {
  id: string;
  node: BaseNode;
  scale: number;
  selected?: boolean;
  activePinId?: string | null;
  onAddInput?: (id: string) => void;
  onPinClick?: (pinId: string, direction: "input" | "output") => void;
  onPinPointerDown?: (e: React.PointerEvent, pin: PinModel) => void;
  onPointerDown?: (nodeId: string, e: React.PointerEvent) => void;
}



/* ================= Default Node UI ================= */

const DefaultNodeUI: React.FC<NodeProps> = ({
  node,
  activePinId,
  onPinClick,
  onPinPointerDown,
}) => {
  const inputsExec = node.inputs.filter(p => p.type === 'exec');
  const inputsData = node.inputs.filter(p => p.type !== 'exec');
  const outputsExec = node.outputs.filter(p => p.type === 'exec');
  const outputsData = node.outputs.filter(p => p.type !== 'exec');

  return (
    <>
      <div className="flex items-center justify-between gap-2 px-3 py-1.5 text-sm font-semibold bg-white/5 rounded-t border-b border-black/20 text-[#cccccc]">
        <div className="flex items-center gap-2">
          <span>{node.title}</span>
        </div>
        <div className="text-[10px] opacity-40 font-mono uppercase tracking-tighter">
          {node.category}
        </div>
      </div>

      <div className="flex flex-col min-h-[60px]">
        {/* Top Row: Flow Pins (Exec) */}
        {(inputsExec.length > 0 || outputsExec.length > 0) && (
          <div className="flex gap-2 px-2 pt-2 whitespace-nowrap items-start">
            <div className="flex flex-col gap-1 flex-1">
              {inputsExec.map((pin) => (
                <Pin key={pin.id} {...pin} isActive={activePinId === pin.id} onPinClick={onPinClick} onPinPointerDown={onPinPointerDown} />
              ))}
            </div>
            <div className="flex-1" />
            <div className="flex flex-col gap-1 flex-1 items-end">
              {outputsExec.map((pin) => (
                <Pin key={pin.id} {...pin} isActive={activePinId === pin.id} onPinClick={onPinClick} onPinPointerDown={onPinPointerDown} />
              ))}
            </div>
          </div>
        )}

        {/* Middle/Bottom Row: Data Pins (Centered) */}
        <div className="flex-1 flex gap-2 px-2 py-2 whitespace-nowrap items-center">
          <div className="flex flex-col gap-1 flex-1">
            {inputsData.map((pin) => (
              <Pin key={pin.id} {...pin} isActive={activePinId === pin.id} onPinClick={onPinClick} onPinPointerDown={onPinPointerDown} />
            ))}
          </div>
          <div className="flex-1" />
          <div className="flex flex-col gap-1 flex-1 items-end">
            {outputsData.map((pin) => (
              <Pin key={pin.id} {...pin} isActive={activePinId === pin.id} onPinClick={onPinClick} onPinPointerDown={onPinPointerDown} />
            ))}
          </div>
        </div>
      </div>
    </>
  );
};

/* ================= Math Node UI ================= */

const MathNodeUI: React.FC<NodeProps> = ({
  node,
  activePinId,
  onAddInput,
  onPinClick,
  onPinPointerDown,
}) => {
  const inputsExec = node.inputs.filter(p => p.type === 'exec');
  const inputsData = node.inputs.filter(p => p.type !== 'exec');
  const outputsExec = node.outputs.filter(p => p.type === 'exec');
  const outputsData = node.outputs.filter(p => p.type !== 'exec');

  // 优先从 schema 获取 centerSymbol，回退到节点属性
  const schemaCenterSymbol = useSchemaStore((s) => s.getCenterSymbol(node.uiStyle, node.type));
  const centerSymbol = schemaCenterSymbol ?? node.centerSymbol;

  return (
    <div className="relative flex flex-col min-h-full">
      {centerSymbol && (
        <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
          <span className="text-2xl font-bold opacity-30 text-black/40">
            {centerSymbol}
          </span>
        </div>
      )}

      {/* Top Row: Flow Pins (Exec) */}
      {(inputsExec.length > 0 || outputsExec.length > 0) && (
        <div className="flex gap-4 px-2 pt-2 z-10 items-start">
          <div className="flex flex-col gap-1 items-start flex-1">
            {inputsExec.map((pin) => (
              <Pin key={pin.id} {...pin} isActive={activePinId === pin.id} onPinClick={onPinClick} onPinPointerDown={onPinPointerDown} />
            ))}
          </div>
          <div className="flex-1" />
          <div className="flex flex-col gap-1 items-end flex-1">
            {outputsExec.map((pin) => (
              <Pin key={pin.id} {...pin} isActive={activePinId === pin.id} onPinClick={onPinClick} onPinPointerDown={onPinPointerDown} />
            ))}
          </div>
        </div>
      )}

      {/* Middle/Bottom Row: Data Pins (Centered) */}
      <div className="flex-1 flex gap-4 px-2 py-2 items-center z-10">
        <div className="flex flex-col gap-1 items-start flex-1">
          {inputsData.map((pin) => (
            <Pin key={pin.id} {...pin} isActive={activePinId === pin.id} onPinClick={onPinClick} onPinPointerDown={onPinPointerDown} />
          ))}
          {onAddInput && (
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
          )}
        </div>
        <div className="flex-1" />
        <div className="flex flex-col gap-1 items-end flex-1">
          {outputsData.map((pin) => (
            <Pin key={pin.id} {...pin} isActive={activePinId === pin.id} onPinClick={onPinClick} onPinPointerDown={onPinPointerDown} />
          ))}
        </div>
      </div>
    </div>
  );
};

/* ================= Main Dispatcher ================= */

export const Node = React.memo<NodeProps>((props) => {
  const { node, onPointerDown, selected } = props;

  if (!node) return null;

  return (
    <div
      id={node.id}
      data-node-id={node.id}
      className={`absolute select-none rounded shadow-2xl border cursor-move ${selected
        ? "border-[var(--accent-color)] ring-2 ring-[var(--accent-color)]/50 z-30"
        : "border-[#2b2b2b] z-10"
        }`}
      style={{
        minWidth: node.noHeader ? 120 : 160,
        minHeight: node.noHeader ? 60 : undefined,
        transform: `translate3d(${node.position.x}px, ${node.position.y}px, 0)`,
        background: "var(--node-base)",
        // 强制开启硬件加速的抗锯齿，并保持文本清晰
        WebkitFontSmoothing: "antialiased",
        MozOsxFontSmoothing: "grayscale",
      }}
      onPointerDown={(e) => onPointerDown?.(node.id, e)}
    >
      {node.uiStyle === "math" ? (
        <MathNodeUI {...props} node={node} />
      ) : (
        <DefaultNodeUI {...props} node={node} />
      )}
    </div>
  );
}, (prev, next) => {
  // 极致性能优化：节点对象引用变化时重绘
  return (
    prev.selected === next.selected &&
    prev.activePinId === next.activePinId &&
    prev.node === next.node &&
    prev.scale === next.scale
  );
});
