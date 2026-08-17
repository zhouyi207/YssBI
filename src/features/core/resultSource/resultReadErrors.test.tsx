// @vitest-environment happy-dom
import type { ReactNode } from 'react';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ResultReadError } from './components/ResultReadError';
import { usePagedResultRows } from './usePagedResultRows';
import { useResultValue } from './useResultValue';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => `localized:${key}` }),
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let valueState: ReturnType<typeof useResultValue> | undefined;
let pageState: ReturnType<typeof usePagedResultRows> | undefined;

function ValueHarness({ showError = false }: { showError?: boolean }) {
  valueState = useResultValue('42');
  return showError && valueState.error ? <ResultReadError error={valueState.error} /> : null;
}

function PageHarness() {
  pageState = usePagedResultRows('42', 1);
  return null;
}

async function flushAsyncWork() {
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
}

describe('result read machine errors', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(invoke).mockReset();
    valueState = undefined;
    pageState = undefined;
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  async function render(ui: ReactNode) {
    await act(async () => {
      root.render(ui);
      await flushAsyncWork();
    });
  }

  it('stores a stable value fallback for parser failures without parser prose', async () => {
    vi.mocked(invoke).mockResolvedValue({ kind: 'value', value: 7, unexpected: 'private response' });

    await render(<ValueHarness />);

    expect(valueState).toMatchObject({
      loading: false,
      error: { code: 'result_value_read_failed', incidentId: null },
    });
    expect(JSON.stringify(valueState)).not.toContain('Invalid result value');
    expect(JSON.stringify(valueState)).not.toContain('private response');
  });

  it('stores a distinct page fallback for parser failures without parser prose', async () => {
    vi.mocked(invoke).mockResolvedValue({ malformed: 'private page response' });

    await render(<PageHarness />);

    expect(pageState).toMatchObject({
      loading: false,
      error: { code: 'result_page_read_failed', incidentId: null },
    });
    expect(JSON.stringify(pageState)).not.toContain('Invalid result page');
    expect(JSON.stringify(pageState)).not.toContain('private page response');
  });

  it('renders a localized generic error and transport code without raw transport text', async () => {
    vi.mocked(invoke).mockRejectedValue(new Error('private native transport failure'));

    await render(<ValueHarness showError />);

    const alert = host.querySelector('[role="alert"]');
    expect(alert?.textContent).toContain('localized:resultSource.readFailed');
    expect(alert?.textContent).toContain('localized:common.errorCode');
    expect(alert?.textContent).toContain('ipc_transport_failure');
    expect(alert?.textContent).not.toContain('private native transport failure');
    expect(alert?.textContent).not.toContain('localized:common.incidentId');
  });

  it('renders IPC code and incident ID without backend details or synthesized Error.message', async () => {
    vi.mocked(invoke).mockRejectedValue({
      code: 'result_value_unavailable',
      details: { detail: 'private backend detail' },
      incidentId: 'incident-result-42',
    });

    await render(<ValueHarness showError />);

    const text = host.querySelector('[role="alert"]')?.textContent;
    expect(text).toContain('localized:resultSource.readFailed');
    expect(text).toContain('result_value_unavailable');
    expect(text).toContain('localized:common.incidentId');
    expect(text).toContain('incident-result-42');
    expect(text).not.toContain('private backend detail');
    expect(text).not.toContain("IPC command 'get_result_value' failed");
  });
});
