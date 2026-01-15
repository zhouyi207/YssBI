import React, { useRef, useState } from "react";
import { Handle, type HandleProps } from "./Handle";
import { type Position } from "../types";

export interface NodeProps {
  /** 基础身份 */
  id: string;
  type: string; // add / branch / print / custom
  title: string;

  /** 画布位置 */
  position: Position;
  scale: number;
  width?: number;
  height?: number;

  /** 端口 */
  inputs: HandleProps[];
  outputs: HandleProps[];

  /** 节点参数（非连线输入） */
  properties?: Record<string, any>;

  /** 执行逻辑（解释器 / 代码生成用） */
  executor?: {
    language: "js" | "python" | "wasm";
    code: string;
  };

  /** UI */
  ui?: {
    color?: string;
    icon?: string;
    resizable?: boolean;
  };

  /** 元信息 */
  meta?: {
    description?: string;
    category?: string;
    version?: string;
  };
}

export const Node: React.FC<NodeProps> = ({
  id,
  type,
  title,

  position,
  scale,
  width,
  height,

  inputs,
  outputs,

  properties,

  ui,
  meta,
}) => {
  const [pos, setPos] = useState(position);
  const dragging = useRef(false);
  const last = useRef<Position>({ x: 0, y: 0 });

  const onMouseDown = (e: React.MouseEvent) => {
    e.stopPropagation();
    dragging.current = true;
    last.current = { x: e.clientX, y: e.clientY };

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
  };

  const onMouseMove = (e: MouseEvent) => {
    if (!dragging.current) return;

    const dx = (e.clientX - last.current.x) / scale;
    const dy = (e.clientY - last.current.y) / scale;

    setPos((p) => ({
      x: p.x + dx,
      y: p.y + dy,
    }));

    last.current = { x: e.clientX, y: e.clientY };
  };

  const onMouseUp = () => {
    dragging.current = false;
    window.removeEventListener("mousemove", onMouseMove);
    window.removeEventListener("mouseup", onMouseUp);
  };

  return (
    <div
      id={id}
      className="absolute select-none rounded shadow-md border border-gray-300"
      style={{
        width,
        height,
        background: ui?.color ?? "#f9fafb",
        transform: `translate(${pos.x}px, ${pos.y}px)`,
      }}
      onMouseDown={onMouseDown}
    >
      {/* ===== Header ===== */}
      <div className="flex items-center gap-2 px-3 py-1 text-sm font-medium bg-gray-200 rounded-t">
        {ui?.icon && <span>{ui.icon}</span>}
        <span>{title}</span>
      </div>

      {/* ===== Body ===== */}
      <div className="flex gap-2 px-2 py-2 whitespace-nowrap">
        {/* Inputs */}
        <div className="flex flex-col gap-1 flex-1">
          {inputs
            .slice()
            .sort((a, b) => a.order - b.order)
            .map((handle) => (
              <Handle key={handle.id} {...handle} />
            ))}
        </div>

        {/* Center content（属性 / 参数） */}
        <div className="flex-1 text-xs text-gray-600">
          {properties &&
            Object.entries(properties).map(([key, value]) => (
              <div key={key} className="flex justify-between">
                <span>{key}</span>
                <span>{String(value)}</span>
              </div>
            ))}
        </div>

        {/* Outputs */}
        <div className="flex flex-col gap-1 flex-1 items-end">
          {outputs
            .slice()
            .sort((a, b) => a.order - b.order)
            .map((handle) => (
              <Handle key={handle.id} {...handle} />
            ))}
        </div>
      </div>

      {/* ===== Footer / Meta（可选） ===== */}
      {meta?.description && (
        <div className="px-2 py-1 text-[10px] text-gray-400 border-t">
          {meta.description}
        </div>
      )}
    </div>
  );
};
