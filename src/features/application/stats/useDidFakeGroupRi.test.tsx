// @vitest-environment happy-dom

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { normalizeIpcError } from '@/services/ipc';
import type { DidFakeGroupEnginePayload, DidPlaceboFakeGroupBlock } from '@/shared/types/report';
import { useDidFakeGroupRi } from './useDidFakeGroupRi';

const { computeFakeGroupRi } = vi.hoisted(() => ({ computeFakeGroupRi: vi.fn() }));

vi.mock('@/features/application/stats/statsActions', () => ({
  PanelDidService: { computeFakeGroupRi },
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

const engine: DidFakeGroupEnginePayload = {
  endog: [1],
  exog_row_major: [1],
  ncols: 1,
  all_labels: [{ variable: 'Treat×Post' }],
  entity_id: [0],
  time_id: [0],
  post: [1],
  treat: [1],
  did_label: 'Treat×Post',
  observed_coef: 1,
  constant: false,
  cov_type: 'nonrobust',
};

const success = {
  available: true,
  observed_coef: 1,
  n_perm: 20,
  n_perm_valid: 18,
  min_valid_permutations: 10,
  n_entities: 8,
  n_treated_entities: 3,
  p_value_ri: 0.2,
  perm_coef_mean: 0.1,
  perm_coef_std: 0.3,
} as const satisfies DidPlaceboFakeGroupBlock;

type HookState = ReturnType<typeof useDidFakeGroupRi>;
let current: HookState | null = null;

function Harness({ initialResult }: { initialResult: DidPlaceboFakeGroupBlock | null }) {
  current = useDidFakeGroupRi(engine, initialResult);
  return null;
}

describe('useDidFakeGroupRi', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    current = null;
    computeFakeGroupRi.mockReset();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  function renderHook(initialResult: DidPlaceboFakeGroupBlock | null = null) {
    act(() => root.render(<Harness initialResult={initialResult} />));
  }

  async function run() {
    await act(async () => {
      await current?.run();
    });
  }

  it('stores only a fallback code for an unstructured failure', async () => {
    computeFakeGroupRi.mockRejectedValueOnce(new Error('private transport failure'));
    renderHook();

    await run();

    expect(current?.error).toEqual({
      code: 'did_fake_group_request_failed',
      incidentId: null,
    });
    expect(JSON.stringify(current?.error)).not.toContain('private transport failure');
  });

  it('preserves only the IPC code and incident ID', async () => {
    computeFakeGroupRi.mockRejectedValueOnce(normalizeIpcError('compute_panel_did_fake_group_ri', {
      code: 'internal_error',
      details: null,
      incidentId: 'incident-did-42',
    }));
    renderHook();

    await run();

    expect(current?.error).toEqual({
      code: 'internal_error',
      incidentId: 'incident-did-42',
    });
  });

  it('rejects a legacy command result instead of retaining backend prose', async () => {
    computeFakeGroupRi.mockResolvedValueOnce({ ...success, method_note: 'legacy backend prose' });
    renderHook();

    await run();

    expect(current?.display).toBeNull();
    expect(current?.error).toEqual({
      code: 'did_fake_group_invalid_response',
      incidentId: null,
    });
  });

  it('rejects a legacy initial report result', () => {
    const legacy = { ...success, method_note: 'legacy report prose' } as unknown as DidPlaceboFakeGroupBlock;

    renderHook(legacy);

    expect(current?.display).toBeNull();
    expect(current?.error).toEqual({
      code: 'did_fake_group_invalid_initial_result',
      incidentId: null,
    });
  });
});
