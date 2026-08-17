import { describe, expect, it } from 'vitest';
import { getPanelViewLabelKey } from './panelPartModel';

describe('panelPartModel', () => {
  it('maps panel view ids to i18n label keys', () => {
    expect(getPanelViewLabelKey('logs')).toBe('panel.logs');
    expect(getPanelViewLabelKey('output')).toBe('panel.output');
  });
});
