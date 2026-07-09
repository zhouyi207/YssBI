import { describe, expect, it } from 'vitest';
import {
  DEFAULT_PANEL_VIEWS,
  resolvePanelViewComponent,
} from './panelPartModel';

describe('panelPartModel', () => {
  it('resolves active panel view component', () => {
    expect(resolvePanelViewComponent(DEFAULT_PANEL_VIEWS, 'logs')).toBe('LogPanel');
    expect(resolvePanelViewComponent(DEFAULT_PANEL_VIEWS, 'output')).toBe('OutputPanel');
  });

  it('falls back to logs when active view is unknown', () => {
    expect(resolvePanelViewComponent(DEFAULT_PANEL_VIEWS, 'missing')).toBe('LogPanel');
  });
});
