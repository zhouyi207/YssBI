// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { InferenceResultDTO } from '@/shared/types/bayes';
import { normalizeIpcError } from '@/services/ipc';
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import {
  BayesCsvExportButton,
  BayesResultFolderButton,
} from './BayesResultPanels';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  savePathDialog: vi.fn(),
  exportCsv: vi.fn(),
  revealFolder: vi.fn(),
}));

vi.mock('@/services/platform/pathDialog', () => ({ savePathDialog: mocks.savePathDialog }));
vi.mock('@/services/platform/opener', () => ({
  revealPath: mocks.revealFolder,
}));
vi.mock('@/services/bayes', () => ({
  exportBayesArtifactCsv: mocks.exportCsv,
  readBayesAutocorrelationData: vi.fn(),
  readBayesDensityPlotData: vi.fn(),
  readBayesPosteriorPredictive: vi.fn(),
  readBayesTracePlotData: vi.fn(),
}));
vi.mock('@/shared/charts', () => ({
  MultiLineChart: () => null,
  PredictiveIntervalChart: () => null,
}));
vi.mock('./BayesPanels', () => ({
  LatexInline: () => null,
  PanelTitle: () => null,
  formatNumber: (value: number) => String(value),
  latexSymbol: (value: string) => value,
}));
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, values?: Record<string, unknown>) => {
      if (key === 'common.error') return 'Error';
      if (key === 'common.incidentId') return 'Incident ID';
      if (key === 'common.unexpectedError') return 'An unexpected error occurred';
      if (typeof values?.error === 'string') return `${key}: ${values.error}`;
      return key;
    },
  }),
}));

const result: InferenceResultDTO = {
  summaries: [],
  diagnostics: {
    chains: 2,
    drawsPerChain: 10,
    warmup: 5,
    divergences: null,
    maxTreedepthHits: null,
    warnings: [],
  },
  artifactManifest: {
    taskId: 'task-42',
    artifacts: [
      { kind: 'posterior_samples', format: 'arrow_ipc', path: 'results/samples.arrow', rows: null },
    ],
  },
};

async function flushPromises(): Promise<void> {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe('Bayes result action feedback', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.clearAllMocks();
    startProjectLifecycle('project-1');
    mocks.savePathDialog.mockResolvedValue({ ok: true, value: 'C:/exports/posterior.csv' });
    mocks.exportCsv.mockResolvedValue(undefined);
    mocks.revealFolder.mockResolvedValue({ ok: true, value: undefined });
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
    clearProjectLifecycle();
  });

  it('shows an open-folder IPC failure beside its button', async () => {
    mocks.revealFolder.mockRejectedValueOnce(normalizeIpcError('reveal_bayes_result_folder', {
      code: 'bayes_result_reveal_failed',
      details: { debug: 'raw folder failure' },
      incidentId: 'incident-bayes-folder-42',
    }));
    act(() => root.render(<BayesResultFolderButton artifactPath="results/samples.arrow" />));

    act(() => host.querySelector('button')?.dispatchEvent(new MouseEvent('click', { bubbles: true })));
    await flushPromises();

    const alert = host.querySelector<HTMLElement>('[role="alert"]');
    expect(alert?.textContent).toContain('bayes_result_reveal_failed');
    expect(alert?.textContent).toContain('incident-bayes-folder-42');
    expect(alert?.textContent).not.toContain('raw folder failure');
    expect(host.querySelector('button')?.getAttribute('aria-describedby')).toBe(alert?.id);
  });

  it('shows an export IPC failure beside its button', async () => {
    mocks.exportCsv.mockRejectedValueOnce(normalizeIpcError('export_bayes_artifact_csv', {
      code: 'bayes_export_failed',
      details: { debug: 'raw export failure' },
      incidentId: 'incident-bayes-export-42',
    }));
    act(() => root.render(
      <BayesCsvExportButton
        result={result}
        kind="posterior_samples"
        fileName="posterior.csv"
      />,
    ));

    act(() => host.querySelector('button')?.dispatchEvent(new MouseEvent('click', { bubbles: true })));
    await flushPromises();

    const alert = host.querySelector<HTMLElement>('[role="alert"]');
    expect(alert?.textContent).toContain('bayes_export_failed');
    expect(alert?.textContent).toContain('incident-bayes-export-42');
    expect(alert?.textContent).not.toContain('raw export failure');
    expect(host.querySelector('button')?.getAttribute('aria-describedby')).toBe(alert?.id);
  });

  it('shows export success locally', async () => {
    act(() => root.render(
      <BayesCsvExportButton
        result={result}
        kind="posterior_samples"
        fileName="posterior.csv"
      />,
    ));

    act(() => host.querySelector('button')?.dispatchEvent(new MouseEvent('click', { bubbles: true })));
    await flushPromises();

    expect(host.querySelector('[role="status"]')?.textContent).toBe('bayes.results.messages.exportSuccess');
  });
});
