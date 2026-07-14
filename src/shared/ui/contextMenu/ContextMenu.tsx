import React, { useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { addGlobalEventListener } from "@/shared/utils/globalEvent";
import { getOverlayPortalRoot } from "@/shared/ui/overlayPortalRoot";
import { cn } from "@/lib/utils";

export interface ContextMenuPosition {
  x: number;
  y: number;
  /**
   * `point` (default): menu top-left at (x, y) — cursor / right-click menus.
   * `below-end`: below anchor, right edges aligned — VS Code editor toolbar overflow.
   *   x = anchor right, y = anchor bottom.
   */
  placement?: 'point' | 'below-end';
  /** Gap below anchor when `placement` is `below-end`. */
  gap?: number;
}

export interface ContextMenuItem {
  id: string;
  label: string;
  icon?: React.ReactNode;
  disabled?: boolean;
  /** Shown as native tooltip when the item is disabled */
  title?: string;
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
  "fixed z-[1] w-max max-w-[min(13.5rem,calc(100vw-1rem))] overflow-hidden rounded-sm border border-border bg-popover py-0 text-[12px] text-popover-foreground shadow-2xl shadow-black/25 dark:shadow-black/45";

const menuItemClass =
  "h-7 w-full justify-start gap-1.5 rounded-none px-2 text-[12px] font-normal hover:bg-[var(--interactive-hover)] hover:text-foreground";

const menuItemDangerClass =
  "text-red-600 hover:bg-red-500/10 hover:text-red-700 dark:text-red-300 dark:hover:text-red-200";

const DEFAULT_BELOW_END_GAP = 2;

export function resolveContextMenuStyle(position: ContextMenuPosition): React.CSSProperties {
  if (position.placement === 'below-end') {
    return {
      left: position.x,
      top: position.y + (position.gap ?? DEFAULT_BELOW_END_GAP),
      transform: 'translateX(-100%)',
    };
  }
  return { left: position.x, top: position.y };
}

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
      style={resolveContextMenuStyle(position)}
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
              title={item.disabled ? item.title : undefined}
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
              <span className="truncate text-left">{item.label}</span>
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
    getOverlayPortalRoot(),
  );
};
