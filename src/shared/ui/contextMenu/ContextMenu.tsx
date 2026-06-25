import React, { useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { addGlobalEventListener } from "@/shared/utils/globalEvent";
import { cn } from "@/lib/utils";

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

/** Project density: tight menu shell (see .cursor/rules/context-menu-density.mdc). */
const menuShellClass =
  "fixed z-[200] min-w-[190px] overflow-hidden rounded-sm border border-border bg-popover/95 py-0 text-[12px] text-popover-foreground shadow-2xl shadow-black/25 backdrop-blur-md dark:shadow-black/45";

const menuItemClass =
  "h-7 w-full justify-start gap-2 rounded-none px-2.5 text-[12px] font-normal hover:bg-[var(--interactive-hover)] hover:text-foreground";

const menuItemDangerClass =
  "text-red-600 hover:bg-red-500/10 hover:text-red-700 dark:text-red-300 dark:hover:text-red-200";

/**
 * Programmatic context menu at screen coordinates.
 * Uses a fixed-position portal (not Radix ContextMenu controlled open) so the menu
 * always anchors to the cursor — Radix ContextMenu cannot position when `open` is
 * set before the user interacts with the trigger.
 */
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
      role="menu"
      className={menuShellClass}
      style={{ left: position.x, top: position.y }}
      onPointerDown={(e) => e.stopPropagation()}
      onClick={(e) => e.stopPropagation()}
    >
      {sections.map((section, sectionIdx) => (
        <React.Fragment key={sectionIdx}>
          {sectionIdx > 0 && <Separator className="my-0 h-px bg-[var(--sidebar-divider)]" />}
          {section.items.map((item) => (
            <Button
              key={item.id}
              type="button"
              variant="ghost"
              role="menuitem"
              disabled={item.disabled}
              className={cn(
                menuItemClass,
                item.danger && menuItemDangerClass,
                item.disabled && "pointer-events-none opacity-40",
              )}
              onMouseDown={(e) => {
                if (e.button !== 0) return;
                e.preventDefault();
                e.stopPropagation();
                if (item.disabled) return;
                item.onClick?.();
                onClose();
              }}
            >
              <span className="flex h-4 w-4 shrink-0 items-center justify-center text-muted-foreground">
                {item.icon ?? null}
              </span>
              <span className="min-w-0 flex-1 truncate text-left">{item.label}</span>
              {item.shortcut && (
                <span className="ml-6 shrink-0 text-[10px] tracking-wide text-muted-foreground">
                  {item.shortcut}
                </span>
              )}
            </Button>
          ))}
        </React.Fragment>
      ))}
    </div>,
    document.body,
  );
};
