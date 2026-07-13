/** Default expanded state when a section key is absent from persisted storage. */
export const SIDEBAR_SECTION_DEFAULTS = {
  graphsEvent: true,
  graphsFunction: false,
  variablesLocal: true,
  variablesGlobal: false,
  dataData: true,
  chartsWorksheets: true,
  commandsUndo: true,
  commandsRedo: false,
} as const;

export type SidebarSectionKey = keyof typeof SIDEBAR_SECTION_DEFAULTS;

/** Resolve whether a sidebar section is expanded from store state + defaults. */
export function resolveSectionExpanded(
  expandedSections: Record<string, boolean>,
  key: SidebarSectionKey,
): boolean {
  if (key in expandedSections) return expandedSections[key];
  return SIDEBAR_SECTION_DEFAULTS[key] ?? false;
}

/** Merge persisted section flags with defaults (no accordion coercion). */
export function mergeExpandedSections(
  expandedSections: Record<string, boolean>,
): Record<string, boolean> {
  return { ...SIDEBAR_SECTION_DEFAULTS, ...expandedSections };
}
