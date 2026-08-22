// @vitest-environment happy-dom
import { act, type ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { uiStore } from '@/features/core/ui/UIStore';
import { normalizeIpcError } from '@/services/ipc';
import { JuliaMenuButton } from './JuliaMenuButton';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const runtime = vi.hoisted(() => ({
  getWorkerStatus: vi.fn(),
  install: vi.fn(),
}));
const translate = vi.hoisted(() => (
  (key: string, values?: Record<string, unknown>) => {
    if (key === 'common.error') return 'Error';
    if (key === 'common.incidentId') return 'Incident ID';
    if (key === 'common.unexpectedError') return 'An unexpected error occurred';
    if (typeof values?.error === 'string') return `${key}: ${values.error}`;
    return key;
  }
));

vi.mock('@/services/julia/juliaRuntimeService', () => ({
  JuliaRuntimeService: {
    getWorkerStatus: runtime.getWorkerStatus,
    install: runtime.install,
  },
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: translate }),
}));

vi.mock('@/components/ui/dropdown-menu', () => ({
  DropdownMenu: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DropdownMenuTrigger: ({ children }: { children: ReactNode }) => <>{children}</>,
  DropdownMenuContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DropdownMenuLabel: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DropdownMenuSeparator: () => <hr />,
  DropdownMenuItem: ({
    children,
    disabled,
    onSelect,
    className,
  }: {
    children: ReactNode;
    disabled?: boolean;
    onSelect?: () => void;
    className?: string;
  }) => (
    <button type="button" disabled={disabled} className={className} onClick={onSelect}>
      {children}
    </button>
  ),
}));

const unavailableStatus = {
  runtimeState: 'missing' as const,
  environmentState: 'missing' as const,
  processState: 'stopped' as const,
  projectDir: '',
};

async function flushPromises(): Promise<void> {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe('JuliaMenuButton feedback', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.clearAllMocks();
    runtime.getWorkerStatus.mockResolvedValue(unavailableStatus);
    runtime.install.mockResolvedValue({
      state: 'ready',
      version: '1.12.0',
      installDir: null,
    });
    vi.spyOn(uiStore, 'confirm').mockResolvedValue(true);
    vi.spyOn(uiStore, 'alert').mockResolvedValue(undefined);
    vi.spyOn(uiStore, 'startProgress').mockImplementation(() => undefined);
    vi.spyOn(uiStore, 'finishProgress').mockImplementation(() => undefined);
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
    vi.restoreAllMocks();
  });

  function render(): void {
    act(() => root.render(<JuliaMenuButton onOpenBayes={vi.fn()} />));
  }

  it('renders a status IPC failure inside the menu without raw details', async () => {
    runtime.getWorkerStatus.mockRejectedValueOnce(normalizeIpcError('get_julia_worker_status', {
      code: 'julia_status_failed',
      details: { debug: 'raw Julia status failure' },
      incidentId: 'incident-julia-status-42',
    }));

    render();
    await flushPromises();

    expect(host.textContent).toContain('julia_status_failed');
    expect(host.textContent).toContain('incident-julia-status-42');
    expect(host.textContent).not.toContain('raw Julia status failure');
  });

  it('opens a MessageDialog request for an install IPC failure', async () => {
    runtime.install.mockRejectedValueOnce(normalizeIpcError('install_julia_runtime', {
      code: 'julia_install_failed',
      details: { debug: 'raw Julia install failure' },
      incidentId: 'incident-julia-install-42',
    }));
    const alert = vi.mocked(uiStore.alert);
    render();
    await flushPromises();

    const install = [...host.querySelectorAll('button')].find((button) => (
      button.textContent === 'julia.menu.install'
    ));
    act(() => install?.dispatchEvent(new MouseEvent('click', { bubbles: true })));
    await flushPromises();

    expect(alert).toHaveBeenCalledWith(expect.objectContaining({
      type: 'error',
      incidentId: 'incident-julia-install-42',
      incidentLabel: 'Incident ID',
    }));
    const options = alert.mock.calls[0]?.[0];
    expect(options?.message).toContain('julia_install_failed');
    expect(options?.message).not.toContain('raw Julia install failure');
  });

  it('does not show success feedback after installation', async () => {
    const alert = vi.mocked(uiStore.alert);
    render();
    await flushPromises();

    const install = [...host.querySelectorAll('button')].find((button) => (
      button.textContent === 'julia.menu.install'
    ));
    act(() => install?.dispatchEvent(new MouseEvent('click', { bubbles: true })));
    await flushPromises();

    expect(alert).not.toHaveBeenCalled();
  });
});
