import type { ReactNode } from "react";

import { cn } from "@/lib/utils";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

export interface WorkbenchStatusBarItem {
  readonly id: string;
  readonly content: ReactNode;
  readonly ariaLabel?: string;
  readonly tooltip?: string;
  readonly onClick?: () => void;
  readonly className?: string;
}

export function StatusBarItem({ item }: { item: WorkbenchStatusBarItem }) {
  const interactive = Boolean(item.onClick);
  const accessibleName = item.ariaLabel ?? item.tooltip;
  const cell = (
    <div
      role={interactive ? "button" : undefined}
      aria-label={interactive ? accessibleName : undefined}
      tabIndex={interactive ? 0 : undefined}
      onClick={item.onClick}
      onKeyDown={
        interactive
          ? (e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                item.onClick?.();
              }
            }
          : undefined
      }
      className={cn(
        "flex h-full items-center gap-1.5 px-2 text-muted-foreground transition-colors",
        interactive &&
          "cursor-pointer outline-none hover:bg-(--hover-bg) hover:text-foreground focus-visible:bg-(--hover-bg) focus-visible:text-foreground focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring",
        item.className,
      )}
    >
      {item.content}
    </div>
  );

  if (!item.tooltip) return cell;

  return (
    <Tooltip>
      <TooltipTrigger asChild>{cell}</TooltipTrigger>
      <TooltipContent side="top">{item.tooltip}</TooltipContent>
    </Tooltip>
  );
}
