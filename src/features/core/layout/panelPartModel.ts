/** VS Code-style bottom panel views (Logs / Output / Terminal). */
export type PanelViewId = 'logs' | 'output';

export interface PanelViewDescriptor {
  id: PanelViewId;
  /** viewRegistry component id */
  component: string;
}

export const DEFAULT_PANEL_VIEWS: PanelViewDescriptor[] = [
  { id: 'logs', component: 'LogPanel' },
  { id: 'output', component: 'OutputPanel' },
];

export function resolvePanelViewComponent(
  views: PanelViewDescriptor[] | undefined,
  activeViewId: string | undefined,
): string {
  const list = views?.length ? views : DEFAULT_PANEL_VIEWS;
  const active = list.find((view) => view.id === activeViewId);
  return active?.component ?? list[0]?.component ?? 'LogPanel';
}
