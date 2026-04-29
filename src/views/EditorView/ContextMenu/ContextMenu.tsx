import React, { useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
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
    <Card
      ref={menuRef}
      className="fixed z-[200] min-w-[180px] py-1 shadow-xl"
      style={{ left: position.x, top: position.y }}
      onPointerDown={(e) => e.stopPropagation()}
    >
      {sections.map((section, sectionIdx) => (
        <React.Fragment key={sectionIdx}>
          {sectionIdx > 0 && (
            <Separator className="my-1" />
          )}
          {section.items.map((item) => (
            <Button
              key={item.id}
              type="button"
              variant={item.danger ? "destructive" : "ghost"}
              size="sm"
              disabled={item.disabled}
              className="h-auto w-full justify-start gap-2 rounded-none px-3 py-1.5 text-xs"
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
                <span className="ml-4 text-[10px] text-muted-foreground">
                  {item.shortcut}
                </span>
              )}
            </Button>
          ))}
        </React.Fragment>
      ))}
    </Card>,
    document.body
  );
};
