import { useEffect, useState, useRef, useCallback } from "react";
import { useEditorGroup } from "@/features/application/editor";
import { useViewportStore } from "@/features/core/viewport";



export  function HUD() {
  const { groupId } = useEditorGroup();

  // Use individual selectors to avoid new object reference on every store update
  const x = useViewportStore(state => state.viewports[groupId]?.x || 0);
  const y = useViewportStore(state => state.viewports[groupId]?.y || 0);
  const scale = useViewportStore(state => state.viewports[groupId]?.scale || 1);

  const [showHUD, setShowHUD] = useState(false);
  const hudTimer = useRef<number | null>(null);

  const triggerHUD = useCallback(() => {
    setShowHUD(true);
    if (hudTimer.current) window.clearTimeout(hudTimer.current);
    hudTimer.current = window.setTimeout(() => {
      setShowHUD(false);
    }, 1000);
  }, []); // Stable callback

  // Trigger HUD on viewport changes
  useEffect(() => {
    triggerHUD();
  }, [x, y, scale, triggerHUD]);

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
      <div>X: {x.toFixed(0)}</div>
      <div>Y: {y.toFixed(0)}</div>
      <div>Zoom: {(scale * 100).toFixed(0)}%</div>
    </div>
  );
}
