import type { CSSProperties } from "react";
import { cn } from "@/lib/utils";

/** Standard sidebar row icon size (list items, section add buttons, command rows). */
export const SIDEBAR_ROW_ICON_SIZE = 12 as const;

/** Fixed leading column width — chevron or row icon (12px). */
export const SIDEBAR_ROW_LEADING_SLOT_CLASS =
  "flex size-3 shrink-0 items-center justify-center" as const;

/** Standard row height (28px) — must match SIDEBAR_FLAT_ROW_HEIGHT. */
export const SIDEBAR_ROW_HEIGHT_CLASS = "h-7" as const;

/** Fixed trailing slot for group headers (add button or spacer). */
export const SIDEBAR_ROW_TRAILING_SLOT_CLASS = "size-6 shrink-0" as const;

/** Left indent for sidebar rows (16px base + 16px per depth level). */
export function sidebarItemIndent(depth = 0): CSSProperties {
  return { paddingLeft: 16 + depth * 16 };
}

function sidebarItemRowBaseClass(isSelected = false) {
  return cn(
    SIDEBAR_ROW_HEIGHT_CLASS,
    "group flex w-full items-center gap-2 pr-2 transition-colors duration-150 ease-out",
    isSelected
      ? "bg-[var(--sidebar-item-active)] text-sidebar-foreground"
      : "hover:bg-[var(--sidebar-hover)] text-sidebar-foreground/70",
  );
}

/** Leaf row: data / graph / variable / worksheet items. */
export function sidebarItemRowClass(isSelected = false) {
  return sidebarItemRowBaseClass(isSelected);
}

/** Collapsible group row — sections and node categories share this layout. */
export function sidebarGroupRowClass() {
  return cn(sidebarItemRowBaseClass(false), "cursor-pointer select-none");
}

export function sidebarItemLabelClass() {
  return "min-w-0 flex-1 truncate text-[12px] leading-normal font-normal tracking-tight";
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

export function sidebarRowActionClass(isSelected = false) {
  return cn(
    "shrink-0 opacity-0 transition-opacity group-hover:opacity-100",
    isSelected ? "text-sidebar-foreground" : "text-muted-foreground",
  );
}

/** Bottom search bar shell in node catalog sidebar. */
export function nodeCatalogSearchShellClass() {
  return "min-w-0 shrink-0 border-t border-border/50 bg-[var(--sidebar-bg)] px-2 py-2";
}
