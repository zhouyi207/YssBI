import { VscAdd } from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import { OverlayScrollbar } from "@/shared/ui/OverlayScrollbar";
import { SidebarChevron } from "./SidebarChevron";
import {
  sidebarCollapsibleHeaderClass,
  sidebarItemIndent,
  sidebarSectionLabelClass,
} from "./sidebarStyles";

type SidebarCollapsibleSectionProps = {
  collapsible?: boolean;
  label: string;
  expanded: boolean;
  onToggle: () => void;
  onAdd?: () => void;
  indentDepth?: number;
  isActive?: boolean;
  leading?: React.ReactNode;
  onHeaderContextMenu?: (e: React.MouseEvent) => void;
  onContentContextMenu?: (e: React.MouseEvent) => void;
  onContextMenu?: (e: React.MouseEvent) => void;
  children: React.ReactNode;
};

export function SidebarCollapsibleSection({
  collapsible = true,
  label,
  expanded,
  onToggle,
  onAdd,
  indentDepth = 0,
  isActive = false,
  leading,
  onHeaderContextMenu,
  onContentContextMenu,
  onContextMenu,
  children,
}: SidebarCollapsibleSectionProps) {
  const headerContextMenu = onHeaderContextMenu ?? onContextMenu;
  const isExpanded = collapsible ? expanded : true;

  const header = (
    <div
      role="button"
      tabIndex={0}
      onClick={(e) => {
        if (!collapsible) return;
        if ((e.target as HTMLElement).closest("[data-add-btn]")) return;
        e.stopPropagation();
        onToggle();
      }}
      onKeyDown={(e) => {
        if (!collapsible) return;
        if (e.key !== "Enter" && e.key !== " ") return;
        e.preventDefault();
        onToggle();
      }}
      onContextMenu={headerContextMenu}
      className={sidebarCollapsibleHeaderClass(isActive)}
      style={sidebarItemIndent(indentDepth)}
    >
      {collapsible ? <SidebarChevron expanded={isExpanded} /> : null}
      {leading ? <span className="flex shrink-0 items-center justify-center">{leading}</span> : null}
      <span className={sidebarSectionLabelClass()}>{label}</span>
      {onAdd ? (
        <Button
          data-add-btn
          type="button"
          variant="ghost"
          size="icon-xs"
          onClick={(e) => {
            e.stopPropagation();
            onAdd();
          }}
          className="shrink-0 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100"
        >
          <VscAdd size={11} />
        </Button>
      ) : null}
    </div>
  );

  return (
    <div
      className={`flex min-h-0 shrink-0 flex-col ${isExpanded ? "flex-1" : "flex-none"}`}
      style={isExpanded ? { minHeight: 0 } : undefined}
    >
      {header}
      <div
        className="grid overflow-hidden transition-[grid-template-rows] duration-150 ease-out"
        style={{ gridTemplateRows: isExpanded ? "1fr" : "0fr" }}
      >
        <OverlayScrollbar className="min-h-0 flex-1">
          <div className="min-h-full" onContextMenu={onContentContextMenu}>
            {children}
          </div>
        </OverlayScrollbar>
      </div>
    </div>
  );
}
