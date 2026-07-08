import { cva } from "class-variance-authority";

/** Editor TabBar shell — aligned with shadcn line TabsList container tokens. */
export const editorTabBarShellClass =
  "box-border flex h-[var(--titlebar-height)] min-h-[var(--titlebar-height)] max-h-[var(--titlebar-height)] w-full shrink-0 select-none items-center overflow-hidden border-b border-border bg-background";

/** Editor TabBar group actions strip (split / close). */
export const editorTabBarActionsClass =
  "flex h-full items-center gap-0.5 border-l border-border bg-background px-1";

/** DnD drop indicator between tabs. */
export const editorTabDropIndicatorClass =
  "pointer-events-none absolute bottom-0 top-0 z-50 w-0.5 bg-primary";

/**
 * VS Code–style editor tab trigger.
 * Custom div (not Radix Tabs) — keeps DnD; visuals mirror shadcn TabsTrigger line variant
 * with a top accent on the active tab.
 */
export const editorTabItemVariants = cva(
  "relative inline-flex h-[var(--titlebar-height)] shrink-0 cursor-pointer items-center gap-2 rounded-none border-r border-border px-3 text-xs font-medium whitespace-nowrap transition-all focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-1 focus-visible:outline-ring",
  {
    variants: {
      active: {
        true: "bg-background text-foreground before:absolute before:inset-x-0 before:top-0 before:h-0.5 before:bg-primary",
        false: "text-muted-foreground hover:bg-muted/50 hover:text-foreground",
      },
      dragging: {
        true: "cursor-grabbing opacity-50",
        false: "",
      },
      preview: {
        true: "italic font-normal",
        false: "",
      },
    },
    defaultVariants: {
      active: false,
      dragging: false,
      preview: false,
    },
  },
);
