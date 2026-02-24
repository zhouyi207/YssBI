import React, { useEffect, useRef } from "react";
import { createPortal } from "react-dom";

export interface ContextMenuPosition {
  x: number;
  y: number;
}

export interface ContextMenuItem {
  id: string;
  label: string;
  icon?: React.ReactNode;
  disabled?: boolean;
  danger?: boolean;
  shortcut?: string;
  onClick?: () => void;
}

export interface ContextMenuSection {
  items: ContextMenuItem[];
}

interface ContextMenuProps {
  position: ContextMenuPosition;
  sections: ContextMenuSection[];
  onClose: () => void;
}

export const ContextMenu: React.FC<ContextMenuProps> = ({
  position,
  sections,
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

  return createPortal(
    <div
      ref={menuRef}
      className="fixed z-[9999] bg-[#252526] rounded-md shadow-xl border border-[#454545] py-1 min-w-[180px]"
      style={{ left: position.x, top: position.y }}
      onPointerDown={(e) => e.stopPropagation()}
    >
      {sections.map((section, sectionIdx) => (
        <React.Fragment key={sectionIdx}>
          {sectionIdx > 0 && (
            <div className="mx-2 my-1 border-t border-[#454545]" />
          )}
          {section.items.map((item) => (
            <button
              key={item.id}
              disabled={item.disabled}
              className={`
                w-full px-3 py-1.5 text-left text-xs flex items-center gap-2 transition-colors
                ${item.disabled
                  ? "text-[#6b6b6b] cursor-default"
                  : item.danger
                    ? "text-[#cccccc] hover:bg-[#cc3333]/20 hover:text-[#f48771]"
                    : "text-[#cccccc] hover:bg-[#094771]"
                }
              `}
              onClick={() => {
                if (item.disabled) return;
                item.onClick?.();
                onClose();
              }}
            >
              <span className="w-4 h-4 flex items-center justify-center shrink-0">
                {item.icon ?? null}
              </span>
              <span className="flex-1">{item.label}</span>
              {item.shortcut && (
                <span className={`text-[10px] ml-4 ${item.disabled ? "text-[#555]" : "text-[#888]"}`}>
                  {item.shortcut}
                </span>
              )}
            </button>
          ))}
        </React.Fragment>
      ))}
    </div>,
    document.body
  );
};
