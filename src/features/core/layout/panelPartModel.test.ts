import { describe, expect, it } from 'vitest';
import {
  DEFAULT_PANEL_VIEWS,
  PANEL_VIEW_SPECS,
  getPanelViewLabelKey,
  resolvePanelViewComponent,
} from './panelPartModel';

describe('panelPartModel', () => {
  it('exposes only implemented views in defaults', () => {
    expect(DEFAULT_PANEL_VIEWS.map((view) => view.id)).toEqual(['logs']);
    expect(PANEL_VIEW_SPECS.output.implemented).toBe(false);
    expect(PANEL_VIEW_SPECS.terminal.implemented).toBe(false);
  });

  it('resolves active panel view component', () => {
    expect(resolvePanelViewComponent(DEFAULT_PANEL_VIEWS, 'logs')).toBe('LogPanel');
    expect(resolvePanelViewComponent(DEFAULT_PANEL_VIEWS, 'output')).toBe('LogPanel');
  });

  it('falls back to logs when active view is unknown or not yet implemented', () => {
    expect(resolvePanelViewComponent(DEFAULT_PANEL_VIEWS, 'missing')).toBe('LogPanel');
    expect(resolvePanelViewComponent(DEFAULT_PANEL_VIEWS, 'terminal')).toBe('LogPanel');
  });

  it('maps panel view ids to i18n label keys', () => {
    expect(getPanelViewLabelKey('logs')).toBe('panel.logs');
    expect(getPanelViewLabelKey('output')).toBe('panel.output');
  });
});
