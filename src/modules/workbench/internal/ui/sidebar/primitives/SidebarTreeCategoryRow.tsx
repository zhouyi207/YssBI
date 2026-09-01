import type { MouseEvent, ReactNode } from "react";
import { VscFolder, VscFolderOpened } from "react-icons/vsc";
import { Collapsible, CollapsibleTrigger } from "@/components/ui/collapsible";
import { cn } from "@/lib/utils";
import { SidebarChevron } from "./SidebarChevron";
import {
  SIDEBAR_ROW_ICON_SIZE,
  SIDEBAR_ROW_LEADING_SLOT_CLASS,
  sidebarGroupRowClass,
  sidebarItemIndent,
} from "./sidebarStyles";

export interface SidebarTreeCategoryRowProps {
  categoryId: string;
  label: ReactNode;
  depth: number;
  expanded: boolean;
  interactionDisabled?: boolean;
  onExpandedChange: (expanded: boolean) => void;
  trailing?: ReactNode;
  onContextMenu?: (event: MouseEvent) => void;
  dataAttributes?: Record<string, string | number | undefined>;
}

export function SidebarTreeCategoryRow({
  categoryId,
  label,
  depth,
  expanded,
  interactionDisabled = false,
  onExpandedChange,
  trailing,
  onContextMenu,
  dataAttributes,
}: SidebarTreeCategoryRowProps) {
  const FolderIcon = expanded ? VscFolderOpened : VscFolder;

  return (
    <Collapsible open={expanded} disabled={interactionDisabled} onOpenChange={onExpandedChange}>
      <div
        className={cn(
          sidebarGroupRowClass(),
          "gap-1 rounded-md text-left transition-none hover:bg-sidebar-accent/50 hover:text-sidebar-foreground",
          expanded ? "text-sidebar-foreground" : "text-sidebar-foreground/75",
        )}
        style={sidebarItemIndent(depth)}
        onContextMenu={onContextMenu}
      >
        <CollapsibleTrigger asChild>
          <button
            type="button"
            disabled={interactionDisabled}
            aria-disabled={interactionDisabled || undefined}
            data-sidebar-tree-category-id={categoryId}
            {...dataAttributes}
            className="flex min-w-0 flex-1 items-center gap-1 self-stretch text-left"
          >
            <span className={SIDEBAR_ROW_LEADING_SLOT_CLASS}>
              <SidebarChevron expanded={expanded} />
            </span>
            <span className={SIDEBAR_ROW_LEADING_SLOT_CLASS} aria-hidden>
              <FolderIcon
                size={SIDEBAR_ROW_ICON_SIZE}
                className={cn(
                  "transition-colors",
                  expanded ? "text-sidebar-primary" : "text-sidebar-foreground/55",
                )}
              />
            </span>
            <span className="min-w-0 flex-1 truncate text-left text-[12px] leading-normal font-medium tracking-tight">
              {label}
            </span>
          </button>
        </CollapsibleTrigger>
        {trailing}
      </div>
    </Collapsible>
  );
}
