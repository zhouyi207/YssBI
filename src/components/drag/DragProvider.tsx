// drag/DragProvider.tsx
import React, { useEffect, useState } from "react";
import { DragContext } from "./DragContext";
import { DragState } from "./type";

export const DragProvider: React.FC<{ children: React.ReactNode }> = ({
  children,
}) => {
  const [drag, setDrag] = useState<DragState>(null);

  useEffect(() => {
    if (!drag) return;

    const onMove = (e: PointerEvent) => {
      setDrag((d) =>
        d
          ? {
              ...d,
              x: e.clientX,
              y: e.clientY,
            }
          : null
      );
    };

    const onUp = () => {
      setDrag(null);
    };

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);

    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
  }, [drag]);

  return (
    <DragContext.Provider
      value={{
        drag,
        startDrag: setDrag,
        updatePosition: (x, y) => setDrag((d) => (d ? { ...d, x, y } : null)),
        endDrag: () => setDrag(null),
      }}
    >
      {children}
    </DragContext.Provider>
  );
};
