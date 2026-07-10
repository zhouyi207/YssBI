/** VS Code-style bottom panel views. Only `implemented` views appear in the tab strip. */
export type PanelViewId = 'logs' | 'output' | 'terminal';

export interface PanelViewDescriptor {
  id: PanelViewId;
  /** viewRegistry component id */
  component: string;
}

export interface PanelViewSpec {
  component: string;
  labelKey: string;
  /** When false, omitted from DEFAULT_PANEL_VIEWS until backend/UI exists. */
  implemented: boolean;
}

export const PANEL_VIEW_SPECS: Record<PanelViewId, PanelViewSpec> = {
  logs: { component: 'LogPanel', labelKey: 'panel.logs', implemented: true },
  output: { component: 'OutputPanel', labelKey: 'panel.output', implemented: true },
  terminal: { component: 'TerminalPanel', labelKey: 'panel.terminal', implemented: false },
};

export const DEFAULT_PANEL_VIEWS: PanelViewDescriptor[] = (
  Object.entries(PANEL_VIEW_SPECS) as [PanelViewId, PanelViewSpec][]
)
  .filter(([, spec]) => spec.implemented)
  .map(([id, spec]) => ({ id, component: spec.component }));

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

  const spec = PANEL_VIEW_SPECS[activeViewId as PanelViewId];
  if (spec?.implemented) return spec.component;

  return list[0]?.component ?? PANEL_VIEW_SPECS.logs.component;
}
