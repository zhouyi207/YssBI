import React, { useRef, useState } from "react";
import { Node } from "./Node";
import { clamp, type Position } from "../types";

/* ================= Canvas ================= */

export default function InfiniteCanvas() {
  const [offset, setOffset] = useState<Position>({ x: 0, y: 0 });
  const [scale, setScale] = useState(1);

  /* ===== HUD ===== */
  const [showHUD, setShowHUD] = useState(false);
  const hudTimer = useRef<number | null>(null);

  const triggerHUD = () => {
    setShowHUD(true);
    if (hudTimer.current) window.clearTimeout(hudTimer.current);
    hudTimer.current = window.setTimeout(() => {
      setShowHUD(false);
    }, 1000);
  };

  /* ===== Canvas Drag ===== */

  const draggingCanvas = useRef(false);
  const last = useRef<Position>({ x: 0, y: 0 });

  const onCanvasMouseDown = (e: React.MouseEvent) => {
    draggingCanvas.current = true;
    last.current = { x: e.clientX, y: e.clientY };

    triggerHUD();

    window.addEventListener("mousemove", onCanvasMouseMove);
    window.addEventListener("mouseup", onCanvasMouseUp);
  };

  const onCanvasMouseMove = (e: MouseEvent) => {
    if (!draggingCanvas.current) return;

    const dx = e.clientX - last.current.x;
    const dy = e.clientY - last.current.y;

    setOffset((o) => ({ x: o.x + dx, y: o.y + dy }));
    last.current = { x: e.clientX, y: e.clientY };

    triggerHUD();
  };

  const onCanvasMouseUp = () => {
    draggingCanvas.current = false;
    window.removeEventListener("mousemove", onCanvasMouseMove);
    window.removeEventListener("mouseup", onCanvasMouseUp);
  };

  /* ===== Zoom ===== */

  const onWheel = (e: React.WheelEvent) => {
    e.preventDefault();

    const factor = 0.001;
    const nextScale = clamp(scale * (1 - e.deltaY * factor), 0.2, 4);

    const rect = e.currentTarget.getBoundingClientRect();
    const mouse = {
      x: e.clientX - rect.left,
      y: e.clientY - rect.top,
    };

    const worldX = (mouse.x - offset.x) / scale;
    const worldY = (mouse.y - offset.y) / scale;

    setScale(nextScale);
    setOffset({
      x: mouse.x - worldX * nextScale,
      y: mouse.y - worldY * nextScale,
    });

    triggerHUD();
  };

  const GRID = 40;

  return (
    <div
      className="relative w-full h-full overflow-hidden bg-gray-900 select-none"
      onWheel={onWheel}
    >
      {/* ================= Grid ================= */}
      <div
        className="absolute inset-0 pointer-events-none"
        style={{
          backgroundImage: `
            linear-gradient(#333 1px, transparent 1px),
            linear-gradient(90deg, #333 1px, transparent 1px)
          `,
          backgroundSize: `${GRID * scale}px ${GRID * scale}px`,
          backgroundPosition: `${-offset.x}px ${-offset.y}px`,
        }}
      />

      {/* ================= World ================= */}
      <div
        className="absolute inset-0 cursor-grab"
        onMouseDown={onCanvasMouseDown}
      >
        <div
          style={{
            transform: `translate(${offset.x}px, ${offset.y}px) scale(${scale})`,
            transformOrigin: "0 0",
          }}
        >
          <Node
            id="node-1"
            type="add"
            title="Add"
            position={{ x: 0, y: 0 }}
            scale={scale}
            inputs={[
              {
                id: "node-1-input-1",
                componentId: "node-1",
                direction: "input",
                kind: "data",
                dataType: "number",
                acceptTypes: ["number"],
                order: 0,
                ui: {
                  color: "#3b82f6",
                },
                meta: {
                  description: "Input 1",
                },
                title: "Input 1",
                connectionCount: 0,
              },
              {
                id: "node-1-input-2",
                componentId: "node-1",
                direction: "input",
                kind: "data",
                dataType: "number",
                acceptTypes: ["number"],
                order: 1,
                ui: {
                  color: "#3b82f6",
                },
                meta: {
                  description: "Input 2",
                },
                title: "Input 2",
                connectionCount: 0,
              },
            ]}
            outputs={[]}
          />
          <Node
            id="node-2"
            type="branch"
            title="Branch"
            position={{ x: 200, y: 120 }}
            scale={scale}
            inputs={[]}
            outputs={[]}
          />
        </div>
      </div>

      {/* ================= HUD ================= */}
      <div
        className={`
          absolute left-3 bottom-3 px-3 py-2
          rounded bg-black/70 text-xs text-gray-200
          transition-opacity duration-500
          ${showHUD ? "opacity-100" : "opacity-0"}
        `}
      >
        <div>X: {offset.x.toFixed(0)}</div>
        <div>Y: {offset.y.toFixed(0)}</div>
        <div>Zoom: {(scale * 100).toFixed(0)}%</div>
      </div>
    </div>
  );
}
