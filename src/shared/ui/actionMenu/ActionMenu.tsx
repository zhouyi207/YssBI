import * as React from 'react';
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuShortcut,
  ContextMenuTrigger,
} from '@/components/ui/context-menu';
import { getOverlayPortalRoot } from '@/shared/ui/overlayPortalRoot';

export interface ActionMenuPosition {
  x: number;
  y: number;
}

export interface ActionMenuItem {
  id: string;
  label: string;
  icon?: React.ReactNode;
  disabled?: boolean;
  title?: string;
  danger?: boolean;
  shortcut?: string;
  onClick?: () => void;
}

export interface ActionMenuSection {
  items: ActionMenuItem[];
}

interface ActionMenuProps {
  position: ActionMenuPosition;
  sections: ActionMenuSection[];
  onClose: () => void;
}

const actionMenuContentClass =
  'max-w-[min(13.5rem,calc(100vw-1rem))]';

const actionMenuItemClass =
  'w-full justify-start';

const actionMenuIconClass =
  'flex size-3.5 shrink-0 items-center justify-center text-muted-foreground';

/**
 * Programmatic action menu backed by shadcn/Radix ContextMenu.
 *
 * Existing callers already own the right-click event and its screen
 * coordinates. Radix owns focus, keyboard navigation, and dismissal; the
 * hidden trigger only bridges those coordinates into its native positioning
 * model.
 */
export function ActionMenu({ position, sections, onClose }: ActionMenuProps) {
  const triggerRef = React.useRef<HTMLSpanElement>(null);
  const [open, setOpen] = React.useState(false);
  const visibleSections = React.useMemo(
    () => sections.filter((section) => section.items.length > 0),
    [sections],
  );

  React.useEffect(() => {
    if (visibleSections.length === 0) return;
    const trigger = triggerRef.current;
    if (!trigger) return;

    trigger.dispatchEvent(new MouseEvent('contextmenu', {
      bubbles: true,
      cancelable: true,
      clientX: position.x,
      clientY: position.y,
    }));
  }, [position.x, position.y, visibleSections.length]);

  const handleOpenChange = React.useCallback((nextOpen: boolean) => {
    setOpen(nextOpen);
    if (!nextOpen) onClose();
  }, [onClose]);

  if (visibleSections.length === 0) return null;

  return (
    <ContextMenu open={open} modal={false} onOpenChange={handleOpenChange}>
      <ContextMenuTrigger
        ref={triggerRef}
        aria-hidden="true"
        data-action-menu-trigger
        className="pointer-events-none fixed left-0 top-0 size-px opacity-0"
        onContextMenu={(event) => event.stopPropagation()}
      />
      <ContextMenuContent
        container={getOverlayPortalRoot()}
        className={actionMenuContentClass}
        onPointerDown={(event) => event.stopPropagation()}
        onCloseAutoFocus={(event) => event.preventDefault()}
      >
        {visibleSections.map((section, sectionIndex) => (
          <React.Fragment key={`section-${sectionIndex}`}>
            {sectionIndex > 0 ? (
              <ContextMenuSeparator />
            ) : null}
            {section.items.map((item) => (
              <ContextMenuItem
                key={item.id}
                disabled={item.disabled}
                title={item.disabled ? item.title : undefined}
                variant={item.danger ? 'destructive' : 'default'}
                className={actionMenuItemClass}
                onSelect={(event) => {
                  if (item.disabled) {
                    event.preventDefault();
                    return;
                  }
                  item.onClick?.();
                }}
              >
                <span className={actionMenuIconClass}>{item.icon ?? null}</span>
                <span className="min-w-0 flex-1 truncate text-left">{item.label}</span>
                {item.shortcut ? (
                  <ContextMenuShortcut>{item.shortcut}</ContextMenuShortcut>
                ) : null}
              </ContextMenuItem>
            ))}
          </React.Fragment>
        ))}
      </ContextMenuContent>
    </ContextMenu>
  );
}
