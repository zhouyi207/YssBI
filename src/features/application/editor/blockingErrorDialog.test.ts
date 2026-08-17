import { beforeEach, describe, expect, it, vi } from 'vitest';
import { IPC_TRANSPORT_FAILURE_CODE, normalizeIpcError } from '@/services/ipc';
import { showBlockingIpcError } from './blockingErrorDialog';

const alert = vi.hoisted(() => vi.fn(() => Promise.resolve()));

vi.mock('@/features/core/ui/UIStore', () => ({ uiStore: { alert } }));
vi.mock('i18next', () => ({
  default: { t: (key: string) => `localized:${key}` },
}));

describe('blocking IPC error dialog', () => {
  beforeEach(() => vi.clearAllMocks());

  it('presents the IPC code and incident ID without backend details', () => {
    const error = normalizeIpcError('save_project_graph', {
      code: 'graph_revision_conflict',
      details: { debug: 'sensitive backend detail' },
      incidentId: 'incident-save-42',
    });

    showBlockingIpcError(
      error,
      'save_project_graph',
      (code) => `localized:save-failed:${code}`,
    );

    expect(alert).toHaveBeenCalledWith({
      title: 'localized:common.error',
      message: 'localized:save-failed:graph_revision_conflict',
      closeText: 'localized:common.close',
      type: 'error',
      incidentId: 'incident-save-42',
      incidentLabel: 'localized:common.incidentId',
    });
    expect(JSON.stringify(alert.mock.calls)).not.toContain('sensitive backend detail');
  });

  it('maps a generic Error to a stable transport code without its message', () => {
    showBlockingIpcError(
      new Error('sensitive native failure'),
      'save_project_graph',
      (code) => `localized:save-failed:${code}`,
    );

    expect(alert).toHaveBeenCalledWith(expect.objectContaining({
      message: `localized:save-failed:${IPC_TRANSPORT_FAILURE_CODE}`,
      incidentId: null,
    }));
    expect(JSON.stringify(alert.mock.calls)).not.toContain('sensitive native failure');
  });
});
