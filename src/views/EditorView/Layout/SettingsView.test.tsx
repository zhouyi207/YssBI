// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { uiStore } from '@/features/core/ui/UIStore';
import { SettingsView } from './SettingsView';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const computation = vi.hoisted(() => ({
  enabled: true,
  confirmed: { settingsRevision: 3 },
  draft: { absolute: '1e-12', relative: '1e-9', statistics: 'listwise' as const },
  isLoading: false,
  isApplying: false,
  isDirty: false,
  validationError: null as string | null,
  error: null as string | null,
  setDraft: vi.fn(),
  apply: vi.fn(async () => undefined),
  restoreRecommended: vi.fn(),
}));

vi.mock('@/features/application/projectSettings/useProjectComputationSettings', () => ({
  useProjectComputationSettings: () => computation,
}));

vi.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: vi.fn() },
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('@/app/i18n', () => ({
  i18n: { changeLanguage: vi.fn() },
}));

vi.mock('@/shared/ui/OverlayScrollbar', () => ({
  OverlayScrollbar: ({ children }: { children: unknown }) => children,
}));

vi.mock('@/shared/ui', () => ({
  Select: ({ id, value, options, onChange, disabled }: {
    id?: string;
    value: string;
    options: Array<{ label: string; value: string }>;
    onChange(value: string): void;
    disabled?: boolean;
  }) => <select id={id} value={value} disabled={disabled} onChange={(event) => onChange(event.target.value)}>
    {options.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
  </select>,
}));

vi.mock('@/features/core/settings/settingsStore', () => {
  const state = {
    theme: {
      workbenchBackground: '#000000', sidebarBackground: '#000000', accentColor: '#000000',
      gridLines: '#000000', nodeBase: '#000000', connectionLines: '#000000', selectionRegion: '#000000',
      execColor: '#000000', boolColor: '#000000', int32Color: '#000000', int64Color: '#000000',
      float32Color: '#000000', float64Color: '#000000', stringColor: '#000000', dateColor: '#000000',
      datetimeColor: '#000000', categoricalColor: '#000000', objectColor: '#000000', anyColor: '#000000',
      oneofColor: '#000000', dataframeColor: '#000000', dataseriesColor: '#000000', arrayColor: '#000000',
      structColor: '#000000',
    },
    editor: { showGrid: true, autoSave: false, snapToGrid: true, fontSize: 12 },
    appearance: {
      colorTheme: 'Dark Modern (Default)', language: 'en-US', activityBarPosition: 'Left',
      panelPosition: 'Bottom', titleBarStyle: 'custom', smoothScroll: true,
    },
    project: { projectName: '', exportPath: '' },
    isLoading: false,
    updateTheme: vi.fn(), updateEditor: vi.fn(), updateAppearance: vi.fn(), updateProject: vi.fn(),
    resetAllToDefaults: vi.fn(), resetThemeToDefaults: vi.fn(), resetEditorToDefaults: vi.fn(),
    resetAppearanceToDefaults: vi.fn(),
  };
  return { useSettingsStore: (selector: (value: typeof state) => unknown) => selector(state) };
});

function click(element: Element): void {
  act(() => element.dispatchEvent(new MouseEvent('click', { bubbles: true })));
}

describe('SettingsView computation settings', () => {
  let host: HTMLDivElement;
  let root: Root;
  const onRequestClose = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    computation.enabled = true;
    computation.isDirty = false;
    computation.validationError = null;
    computation.error = null;
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  function render(): void {
    act(() => root.render(<SettingsView onRequestClose={onRequestClose} />));
  }

  async function openComputation(): Promise<void> {
    const button = [...host.querySelectorAll('button')].find((item) => (
      item.textContent === 'settings.sections.computation'
    ));
    if (!button) throw new Error('computation section button missing');
    click(button);
    await act(async () => { await Promise.resolve(); });
  }

  it('disables the computation group when no project is open', async () => {
    computation.enabled = false;
    render();
    await openComputation();
    const group = host.querySelector('[role="group"][aria-label="settings.computation.groupLabel"]');
    expect(group?.getAttribute('aria-disabled')).toBe('true');
    expect(group?.querySelectorAll('input:disabled, select:disabled, button:disabled').length).toBeGreaterThan(0);
  });

  it('shows tolerance formula help, Listwise/Reject, Apply, and recommended reset', async () => {
    computation.isDirty = true;
    vi.spyOn(uiStore, 'confirm').mockResolvedValue(true);
    render();
    await openComputation();
    expect(host.textContent).toContain('|a - b| ≤ max(absolute, relative × max(|a|, |b|))');
    expect(host.textContent).toContain('Listwise');
    expect(host.textContent).toContain('Reject');
    click([...host.querySelectorAll('button')].find((item) => item.textContent === 'Restore Recommended Values')!);
    click([...host.querySelectorAll('button')].find((item) => item.textContent === 'Apply')!);
    expect(computation.restoreRecommended).toHaveBeenCalledOnce();
    expect(computation.apply).toHaveBeenCalledOnce();
  });

  it('uses the application confirmation modal before dirty close and section changes', async () => {
    computation.isDirty = true;
    const confirm = vi.spyOn(uiStore, 'confirm').mockResolvedValue(false);
    render();
    await openComputation();

    click(host.querySelector('button[aria-label="Close settings"]')!);
    await act(async () => { await Promise.resolve(); });
    expect(confirm).toHaveBeenCalledWith(expect.objectContaining({ title: 'Discard computation changes?' }));
    expect(onRequestClose).not.toHaveBeenCalled();

    click([...host.querySelectorAll('button')].find((item) => item.textContent === 'settings.sections.editor')!);
    await act(async () => { await Promise.resolve(); });
    expect(confirm).toHaveBeenCalledTimes(2);
    expect(host.textContent).toContain('settings.sections.computation');
  });
});
