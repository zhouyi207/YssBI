import React, { useEffect, useRef } from "react";

interface PinContextMenuProps {
  x: number;
  y: number;
  onRemove: () => void;
  onClose: () => void;
}

export const PinContextMenu: React.FC<PinContextMenuProps> = ({
  x,
  y,
  onRemove,
  onClose,
}) => {
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as HTMLElement)) {
        onClose();
      }
    };
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("pointerdown", handleClickOutside, true);
    window.addEventListener("keydown", handleEscape);
    return () => {
      window.removeEventListener("pointerdown", handleClickOutside, true);
      window.removeEventListener("keydown", handleEscape);
    };
  }, [onClose]);

  return (
    <div
      ref={menuRef}
      className="fixed z-[9999] bg-white rounded-md shadow-lg border border-gray-200 py-1 min-w-[140px]"
      style={{ left: x, top: y }}
      onPointerDown={(e) => e.stopPropagation()}
    >
      <button
        className="w-full px-3 py-1.5 text-left text-xs hover:bg-red-50 hover:text-red-600 transition-colors flex items-center gap-2"
        onClick={() => {
          onRemove();
          onClose();
        }}
      >
        <svg width="12" height="12" viewBox="0 0 12 12" className="shrink-0">
          <path d="M3 3L9 9M9 3L3 9" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
        </svg>
        Remove Pin
      </button>
    </div>
  );
};
