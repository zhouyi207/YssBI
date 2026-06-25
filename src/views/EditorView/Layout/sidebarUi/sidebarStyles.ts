import type { CSSProperties } from "react";
import { cn } from "@/lib/utils";

/** Left indent for sidebar rows (16px base + 16px per depth level). */
export function sidebarItemIndent(depth = 0): CSSProperties {
  return { paddingLeft: 16 + depth * 16 };
}

/** Leaf row: data / graph / variable / worksheet items. */
export function sidebarItemRowClass(isSelected = false) {
  return cn(
    "group flex w-full items-center gap-2 py-1.5 pr-2 transition-colors duration-150 ease-out",
    isSelected
      ? "bg-[var(--sidebar-item-active)] text-sidebar-foreground"
      : "hover:bg-[var(--sidebar-hover)] text-sidebar-foreground/70",
  );
}

/** Collapsible section header row (folders, Event/Data section titles, etc.). */
export function sidebarCollapsibleHeaderClass(isActive = false) {
  return cn(
    sidebarItemRowClass(isActive),
    "w-full shrink-0 cursor-pointer",
  );
}

export function sidebarItemLabelClass() {
  return "min-w-0 flex-1 truncate text-[12px] font-normal tracking-tight";
}

export function sidebarSectionLabelClass() {
  return "min-w-0 flex-1 truncate text-[12px] tracking-tight";
}

export function sidebarRowActionClass(isSelected = false) {
  return cn(
    "shrink-0 opacity-0 transition-opacity group-hover:opacity-100",
    isSelected ? "text-sidebar-foreground" : "text-muted-foreground",
  );
}

export function sidebarChevronClass(expanded: boolean, size: 11 | 12 = 11) {
  return {
    className: "shrink-0 text-muted-foreground transition-transform duration-150 ease-out",
    size,
    expanded,
  } as const;
}
