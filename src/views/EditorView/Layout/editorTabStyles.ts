import { cva } from "class-variance-authority";

/** Editor TabBar shell — VS Code–style muted strip above editor content. */
export const editorTabBarShellClass =
  "box-border flex h-[var(--titlebar-height)] min-h-[var(--titlebar-height)] max-h-[var(--titlebar-height)] w-full shrink-0 select-none items-stretch overflow-hidden border-b border-border bg-muted/25";

/** Tab strip — grows to fill bar. */
export const editorTabBarStripClass =
  "relative flex h-full min-h-full min-w-full flex-1 cursor-default items-stretch overflow-visible";

/** Editor TabBar group actions strip (split / close). */
export const editorTabBarActionsClass =
  "flex h-full shrink-0 items-center gap-0.5 border-l border-border bg-muted/25 px-1";

/** VS Code–style open slot while reordering tabs within / onto a tab strip. */
export const editorTabReorderGapClass =
  "pointer-events-none absolute top-0.5 bottom-0.5 z-[45] rounded-sm border-2 border-primary/70 bg-primary/20 shadow-[inset_0_0_0_1px_hsl(var(--primary)/0.15)] transition-[left,width] duration-150 ease-out";

/**
 * VS Code–style editor tab trigger.
 * Active tab: editor background + bottom primary accent.
 * Inactive: slightly recessed muted background; close icon on group hover.
 */
export const editorTabItemVariants = cva(
  "group/tab relative inline-flex h-[var(--titlebar-height)] shrink-0 cursor-pointer items-center gap-1.5 rounded-none border-r border-border/60 px-3 text-xs leading-none whitespace-nowrap transition-[transform,colors] duration-150 ease-out focus-visible:outline-none",
  {
    variants: {
      active: {
        true: "z-[1] -mb-px border-b-2 border-b-primary bg-background text-foreground",
        false: "bg-muted/20 text-muted-foreground hover:bg-muted/45 hover:text-foreground",
      },
      dragging: {
        true: "invisible",
        false: "",
      },
      preview: {
        true: "italic font-normal",
        false: "font-medium",
      },
    },
    defaultVariants: {
      active: false,
      dragging: false,
      preview: false,
    },
  },
);

/** Close / dirty affordance — hidden until tab row hover (VS Code). */
export const editorTabCloseButtonClass =
  "ml-0.5 h-5 w-5 shrink-0 text-muted-foreground opacity-0 transition-opacity group-hover/tab:opacity-100 hover:text-foreground data-[dirty=true]:opacity-100";
