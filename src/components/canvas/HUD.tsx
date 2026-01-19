import { useEffect, useState, useRef } from "react";
import { useCanvas } from "./CanvasContext";

export default function HUD() {
  const { canvas } = useCanvas();
  const [showHUD, setShowHUD] = useState(false);
  const hudTimer = useRef<number | null>(null);

  const triggerHUD = () => {
    setShowHUD(true);
    if (hudTimer.current) window.clearTimeout(hudTimer.current);
    hudTimer.current = window.setTimeout(() => {
      setShowHUD(false);
    }, 1000);
  };

  useEffect(() => {
    triggerHUD();
  }, [canvas]);

  return (
    <div
      className={`
          hud-container
          absolute left-3 bottom-3 px-3 py-2
          rounded bg-black/70 text-xs text-gray-200
          transition-opacity duration-500
          ${showHUD ? "opacity-100" : "opacity-0"}
        `}
    >
      <div>X: {canvas?.x.toFixed(0)}</div>
      <div>Y: {canvas?.y.toFixed(0)}</div>
      <div>Zoom: {canvas?.scale ? (canvas.scale * 100).toFixed(0) : 0}%</div>
    </div>
  );
}
