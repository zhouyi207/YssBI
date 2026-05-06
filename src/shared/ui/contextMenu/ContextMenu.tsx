import React, { useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { addGlobalEventListener } from "@/shared/utils/globalEvent";

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
    const handleClickOutside = (e: PointerEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as HTMLElement)) {
        onClose();
      }
    };
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    const cleanupPointerDown = addGlobalEventListener(window, "pointerdown", handleClickOutside, true);
    const cleanupKeyDown = addGlobalEventListener(window, "keydown", handleEscape);
    return () => {
      cleanupPointerDown();
      cleanupKeyDown();
    };
  }, [onClose]);

  return createPortal(
    <div
      ref={menuRef}
      className="fixed z-[200] min-w-[190px] overflow-hidden rounded-sm border border-border bg-popover/95 py-0 text-[12px] text-popover-foreground shadow-2xl shadow-black/25 backdrop-blur-md dark:shadow-black/45"
      style={{ left: position.x, top: position.y }}
      onPointerDown={(e) => e.stopPropagation()}
      onClick={(e) => e.stopPropagation()}
    >
      {sections.map((section, sectionIdx) => (
        <React.Fragment key={sectionIdx}>
          {sectionIdx > 0 && (
            <div className="my-0 h-px bg-[var(--sidebar-divider)]" />
          )}
          {section.items.map((item) => (
            <button
              key={item.id}
              type="button"
              disabled={item.disabled}
              className={[
                "group flex h-7 w-full items-center gap-2 px-2.5 text-left outline-none transition-colors",
                "disabled:pointer-events-none disabled:opacity-40",
                item.danger
                  ? "text-red-600 hover:bg-red-500/10 hover:text-red-700 focus-visible:bg-red-500/10 dark:text-red-300 dark:hover:text-red-200"
                  : "hover:bg-[var(--interactive-hover)] hover:text-foreground focus-visible:bg-[var(--interactive-hover)] focus-visible:text-foreground",
              ].join(" ")}
              onClick={() => {
                if (item.disabled) return;
                item.onClick?.();
                onClose();
              }}
            >
              <span className="flex h-4 w-4 shrink-0 items-center justify-center text-muted-foreground transition-colors group-hover:text-current">
                {item.icon ?? null}
              </span>
              <span className="min-w-0 flex-1 truncate">{item.label}</span>
              {item.shortcut && (
                <span className="ml-6 shrink-0 text-[10px] tracking-wide text-muted-foreground">
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
