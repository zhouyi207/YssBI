import type { ChangeEvent } from "react";
import { VscCollapseAll, VscExpandAll, VscSearch } from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

export interface SidebarTreeSearchInputProps {
  value: string;
  onChange: (event: ChangeEvent<HTMLInputElement>) => void;
  placeholder: string;
  expandAllLabel: string;
  collapseAllLabel: string;
  allCategoriesExpanded: boolean;
  canToggleAllCategories: boolean;
  onToggleAllCategories: () => void;
  autoFocus?: boolean;
}

export function SidebarTreeSearchInput({
  value,
  onChange,
  placeholder,
  expandAllLabel,
  collapseAllLabel,
  allCategoriesExpanded,
  canToggleAllCategories,
  onToggleAllCategories,
  autoFocus = false,
}: SidebarTreeSearchInputProps) {
  const toggleLabel = allCategoriesExpanded ? collapseAllLabel : expandAllLabel;
  const ToggleIcon = allCategoriesExpanded ? VscCollapseAll : VscExpandAll;

  return (
    <div data-sidebar-tree-search className="relative">
      <Input
        aria-label={placeholder}
        autoFocus={autoFocus}
        autoComplete="off"
        className="h-8 border-border/60 bg-background/35 pl-8 pr-8 text-xs shadow-none transition-colors focus-visible:bg-background"
        value={value}
        placeholder={placeholder}
        onChange={onChange}
      />
      <span className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-muted-foreground">
        <VscSearch aria-hidden size={13} />
      </span>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            data-sidebar-tree-expand-toggle
            aria-label={toggleLabel}
            disabled={!canToggleAllCategories}
            onClick={onToggleAllCategories}
            className="absolute right-1 top-1/2 -translate-y-1/2 text-muted-foreground"
          >
            <ToggleIcon aria-hidden="true" />
          </Button>
        </TooltipTrigger>
        <TooltipContent side="bottom">{toggleLabel}</TooltipContent>
      </Tooltip>
    </div>
  );
}
