/** Supported workbench panel views. Unsupported placeholders are not part of the model. */
export const PANEL_VIEW_IDS = ['logs', 'output'] as const;
export type PanelViewId = (typeof PANEL_VIEW_IDS)[number];

export interface PanelViewDescriptor {
  id: PanelViewId;
  /** viewRegistry component id */
  component: string;
}

export interface PanelViewSpec {
  component: string;
  labelKey: string;
}

export const PANEL_VIEW_SPECS: Record<PanelViewId, PanelViewSpec> = {
  logs: { component: 'LogPanel', labelKey: 'panel.logs' },
  output: { component: 'OutputPanel', labelKey: 'panel.output' },
};

export const DEFAULT_PANEL_VIEWS: PanelViewDescriptor[] = PANEL_VIEW_IDS.map((id) => ({
  id,
  component: PANEL_VIEW_SPECS[id].component,
}));

export function getPanelViewLabelKey(viewId: PanelViewId): string {
  return PANEL_VIEW_SPECS[viewId]?.labelKey ?? viewId;
}

export function resolvePanelViewComponent(
  views: PanelViewDescriptor[] | undefined,
  activeViewId: string | undefined,
): string {
  const list = views?.length ? views : DEFAULT_PANEL_VIEWS;
  const active = list.find((view) => view.id === activeViewId);
  if (active) return active.component;

  return list[0]?.component ?? PANEL_VIEW_SPECS.logs.component;
}
