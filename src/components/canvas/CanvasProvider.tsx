import React, { useRef, useState, useCallback } from "react";
import { CanvasContext } from "./CanvasContext";
import { CanvasState, Gesture } from "./type";
import { clamp } from "../../types";

export const CanvasProvider: React.FC<{ children: React.ReactNode }> = ({
  children,
}) => {
  const [canvas, setCanvas] = useState<CanvasState>({
    x: 0,
    y: 0,
    scale: 1,
  });

  const gestureRef = useRef<Gesture>(null);
  const [selection, setSelection] = useState<{
    startX: number;
    startY: number;
    currentX: number;
    currentY: number;
  } | null>(null);

  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    visible: boolean;
  } | null>(null);

  /* ================= Wheel Zoom ================= */

  const onCanvasWheel = useCallback(
    (e: React.WheelEvent) => {
      e.preventDefault();

      const factor = 0.001;
      const nextScale = clamp(canvas.scale * (1 - e.deltaY * factor), 0.2, 4);

      const rect = e.currentTarget.getBoundingClientRect();
      const mouseX = e.clientX - rect.left;
      const mouseY = e.clientY - rect.top;

      const worldX = (mouseX - canvas.x) / canvas.scale;
      const worldY = (mouseY - canvas.y) / canvas.scale;

      setCanvas({
        scale: nextScale,
        x: mouseX - worldX * nextScale,
        y: mouseY - worldY * nextScale,
      });
    },
    [canvas]
  );
  /* ================= Pointer Down ================= */

  const onCanvasPointerDown = (e: React.PointerEvent) => {
    if (e.button === 0) {
      // 左键开始选择
      const start = {
        startX: e.clientX,
        startY: e.clientY,
        currentX: e.clientX,
        currentY: e.clientY,
      };
      gestureRef.current = { type: "select", ...start };
      setSelection(start);
    } else if (e.button === 1 || e.button === 2) {
      // 中键或右键开始平移
      gestureRef.current = {
        type: "pan",
        lastX: e.clientX,
        lastY: e.clientY,
        moved: false,
      };
    }

    window.addEventListener("pointermove", onCanvasPointerMove);
    window.addEventListener("pointerup", onCanvasPointerUp);
  };

  /* ================= Pointer Move ================= */

  const onCanvasPointerMove = (e: PointerEvent) => {
    const gesture = gestureRef.current;
    if (!gesture) return;

    if (gesture.type === "pan") {
      const dx = e.clientX - gesture.lastX;
      const dy = e.clientY - gesture.lastY;

      if (Math.abs(dx) > 2 || Math.abs(dy) > 2) {
        gesture.moved = true;
      }

      setCanvas((prev) => ({
        ...prev,
        x: prev.x + dx,
        y: prev.y + dy,
      }));

      gesture.lastX = e.clientX;
      gesture.lastY = e.clientY;
    } else if (gesture.type === "select") {
      gesture.currentX = e.clientX;
      gesture.currentY = e.clientY;
      setSelection({ ...gesture });
    }
  };

  /* ================= Pointer Up ================= */

  const onCanvasPointerUp = (e: PointerEvent) => {
    const gesture = gestureRef.current;

    if (gesture?.type === "select") {
      setSelection(null);
    } else if (gesture?.type === "pan" && !gesture.moved && e.button === 2) {
      setContextMenu({ x: e.clientX, y: e.clientY, visible: true });
    }

    gestureRef.current = null;
    window.removeEventListener("pointermove", onCanvasPointerMove);
    window.removeEventListener("pointerup", onCanvasPointerUp);
  };

  return (
    <CanvasContext.Provider
      value={{
        canvas,
        setCanvas,
        onCanvasWheel,
        onCanvasPointerDown,
        selection,
        contextMenu,
        setContextMenu,
      }}
    >
      {children}
    </CanvasContext.Provider>
  );
};
