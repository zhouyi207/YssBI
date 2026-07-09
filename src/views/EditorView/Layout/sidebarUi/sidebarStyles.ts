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

/** Collapsible section header row (Event/Data section titles, etc.). */
export function sidebarCollapsibleHeaderClass(isActive = false) {
  return cn(
    sidebarItemRowClass(isActive),
    "w-full shrink-0 cursor-pointer",
  );
}

export function sidebarItemLabelClass() {
  return "min-w-0 flex-1 truncate text-[12px] font-normal tracking-tight";
}

/** Variable type badge / command timestamp — capped so rows don't raise sidebar min-content width. */
export function sidebarTrailingMetaClass() {
  return "min-w-0 max-w-[4.5rem] shrink truncate text-[10px] text-muted-foreground/70";
}

export function sidebarVariableTypeBadgeClass(isSelected = false) {
  return cn(
    "min-w-0 max-w-[40%] shrink truncate flex items-center gap-1 px-1 py-0.5 text-[10px] font-normal",
    isSelected ? "bg-white/[0.12]" : "bg-sidebar-accent/50",
  );
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

/** Node catalog category row (matches collapsible section headers). */
export function nodeCatalogCategoryRowClass() {
  return cn(sidebarCollapsibleHeaderClass(false), "cursor-pointer select-none");
}

/** Node catalog leaf row — slightly brighter label, same hover tokens as sidebar items. */
export function nodeCatalogLeafRowClass(isSelected = false) {
  return cn(
    "group flex w-full items-center gap-2 py-1.5 pr-2 transition-colors duration-150 ease-out",
    isSelected
      ? "bg-[var(--sidebar-item-active)] text-sidebar-foreground"
      : "text-sidebar-foreground/90 hover:bg-[var(--sidebar-hover)] hover:text-sidebar-foreground",
  );
}

export function nodeCatalogLeafLabelClass(isSelected = false) {
  return cn(
    "min-w-0 flex-1 truncate text-[13px] font-normal leading-snug",
    isSelected ? "text-sidebar-foreground" : "text-sidebar-foreground/90 group-hover:text-sidebar-foreground",
  );
}

/** Bottom search bar shell in node catalog sidebar. */
export function nodeCatalogSearchShellClass() {
  return "min-w-0 shrink-0 border-t border-border/50 bg-[var(--sidebar-bg)] px-2 py-2";
}
