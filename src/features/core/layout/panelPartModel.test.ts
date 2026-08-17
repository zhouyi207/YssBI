import { describe, expect, it } from 'vitest';
import {
  DEFAULT_PANEL_VIEWS,
  getPanelViewLabelKey,
  resolvePanelViewComponent,
} from './panelPartModel';

describe('panelPartModel', () => {
  it('exposes every supported view in defaults', () => {
    expect(DEFAULT_PANEL_VIEWS.map((view) => view.id)).toEqual(['logs', 'output']);
  });

  it('resolves active panel view component', () => {
    expect(resolvePanelViewComponent(DEFAULT_PANEL_VIEWS, 'logs')).toBe('LogPanel');
    expect(resolvePanelViewComponent(DEFAULT_PANEL_VIEWS, 'output')).toBe('OutputPanel');
  });

  it('falls back to logs when the active view is unknown or removed', () => {
    expect(resolvePanelViewComponent(DEFAULT_PANEL_VIEWS, 'missing')).toBe('LogPanel');
    expect(resolvePanelViewComponent(DEFAULT_PANEL_VIEWS, 'terminal')).toBe('LogPanel');
  });

  it('maps panel view ids to i18n label keys', () => {
    expect(getPanelViewLabelKey('logs')).toBe('panel.logs');
    expect(getPanelViewLabelKey('output')).toBe('panel.output');
  });
});
