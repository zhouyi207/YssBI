import React, { useRef, useState, useCallback } from "react";
import { CanvasContext } from "./CanvasContext";
import { CanvasState, Gesture } from "./type";
import { clamp } from "../../types";
import { Pin } from "../node/models";

export const CanvasProvider: React.FC<{ children: React.ReactNode }> = ({
  children,
}) => {
  const [canvas, setCanvas] = useState<CanvasState>({
    x: 0,
    y: 0,
    scale: 1,
  });

  const [gesture, setGesture] = useState<Gesture>(null);
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

  /* ================= Pointer Move ================= */

  const onCanvasPointerMove = useCallback((e: PointerEvent) => {
    const currentGesture = gestureRef.current;
    if (!currentGesture) return;

    if (currentGesture.type === "pan") {
      const dx = e.clientX - currentGesture.lastX;
      const dy = e.clientY - currentGesture.lastY;

      if (Math.abs(dx) > 2 || Math.abs(dy) > 2) {
        currentGesture.moved = true;
      }

      setCanvas((prev) => ({
        ...prev,
        x: prev.x + dx,
        y: prev.y + dy,
      }));

      currentGesture.lastX = e.clientX;
      currentGesture.lastY = e.clientY;
      setGesture({ ...currentGesture });
    } else if (currentGesture.type === "select") {
      currentGesture.currentX = e.clientX;
      currentGesture.currentY = e.clientY;
      setSelection({ ...currentGesture });
      setGesture({ ...currentGesture });
    } else if (currentGesture.type === "connect") {
      currentGesture.currentX = e.clientX;
      currentGesture.currentY = e.clientY;
      setGesture({ ...currentGesture });
    }
  }, []);

  /* ================= Pointer Up ================= */

  const onCanvasPointerUp = useCallback((e: PointerEvent) => {
    const currentGesture = gestureRef.current;

    if (currentGesture?.type === "select") {
      setSelection(null);
    } else if (currentGesture?.type === "pan" && !currentGesture.moved && e.button === 2) {
      setContextMenu({ x: e.clientX, y: e.clientY, visible: true });
    } else if (currentGesture?.type === "connect") {
      // Logic handled in Canvas.tsx via gesture state
    }

    gestureRef.current = null;
    setGesture(null);
    window.removeEventListener("pointermove", onCanvasPointerMove);
    window.removeEventListener("pointerup", onCanvasPointerUp);
  }, [onCanvasPointerMove]);

  /* ================= Pin Pointer Down ================= */
  const onPinPointerDown = useCallback((e: React.PointerEvent, pin: Pin) => {
    e.stopPropagation();
    e.preventDefault();

    if (e.altKey) {
      setGesture({ type: "disconnect", pin });
      return;
    }

    const start = {
      type: "connect" as const,
      startPin: pin,
      startX: e.clientX,
      startY: e.clientY,
      currentX: e.clientX,
      currentY: e.clientY,
      isReconnect: e.ctrlKey,
    };

    gestureRef.current = start;
    setGesture(start);

    window.addEventListener("pointermove", onCanvasPointerMove);
    window.addEventListener("pointerup", onCanvasPointerUp);
  }, [onCanvasPointerMove, onCanvasPointerUp]);

  /* ================= Pointer Down ================= */

  const onCanvasPointerDown = useCallback((e: React.PointerEvent) => {
    if (e.button === 0) {
      // 左键开始选择
      const start = {
        type: "select" as const,
        startX: e.clientX,
        startY: e.clientY,
        currentX: e.clientX,
        currentY: e.clientY,
      };
      gestureRef.current = start;
      setSelection(start);
      setGesture(start);
    } else if (e.button === 1 || e.button === 2) {
      // 中键或右键开始平移
      const start = {
        type: "pan" as const,
        lastX: e.clientX,
        lastY: e.clientY,
        moved: false,
      };
      gestureRef.current = start;
      setGesture(start);
    }

    window.addEventListener("pointermove", onCanvasPointerMove);
    window.addEventListener("pointerup", onCanvasPointerUp);
  }, [onCanvasPointerMove, onCanvasPointerUp]);

  return (
    <CanvasContext.Provider
      value={{
        canvas,
        setCanvas,
        onCanvasWheel,
        onCanvasPointerDown,
        onPinPointerDown,
        selection,
        gesture,
        setGesture,
        contextMenu,
        setContextMenu,
      }}
    >
      {children}
    </CanvasContext.Provider>
  );
};
