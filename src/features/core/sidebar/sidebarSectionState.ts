/** Default expanded state when a section key is absent from persisted storage. */
export const SIDEBAR_SECTION_DEFAULTS = {
  dataData: true,
} as const;

export type SidebarSectionKey = string;
type SupportedSidebarSectionKey = keyof typeof SIDEBAR_SECTION_DEFAULTS;

export function isSupportedSidebarSectionKey(key: string): key is SupportedSidebarSectionKey {
  return key === "dataData";
}

/** Resolve whether a sidebar section is expanded from store state + defaults. */
export function resolveSectionExpanded(
  expandedSections: Record<string, boolean>,
  key: SidebarSectionKey,
): boolean {
  if (key in expandedSections) return expandedSections[key];
  return key in SIDEBAR_SECTION_DEFAULTS
    ? SIDEBAR_SECTION_DEFAULTS[key as keyof typeof SIDEBAR_SECTION_DEFAULTS]
    : false;
}

/** Merge persisted section flags with defaults (no accordion coercion). */
export function mergeExpandedSections(
  expandedSections: Readonly<Record<string, unknown>>,
): Record<string, boolean> {
  const dataData = expandedSections.dataData;
  return {
    dataData: typeof dataData === "boolean" ? dataData : SIDEBAR_SECTION_DEFAULTS.dataData,
  };
}
