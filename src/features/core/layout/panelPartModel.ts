/** Supported workbench panel views. Unsupported placeholders are not part of the model. */
export const PANEL_VIEW_IDS = ['logs', 'output'] as const;
export type PanelViewId = (typeof PANEL_VIEW_IDS)[number];

export interface PanelViewSpec {
  component: string;
  labelKey: string;
}

export const PANEL_VIEW_SPECS: Record<PanelViewId, PanelViewSpec> = {
  logs: { component: 'LogPanel', labelKey: 'panel.logs' },
  output: { component: 'OutputPanel', labelKey: 'panel.output' },
};

export function getPanelViewLabelKey(viewId: PanelViewId): string {
  return PANEL_VIEW_SPECS[viewId]?.labelKey ?? viewId;
}
