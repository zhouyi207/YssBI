// @vitest-environment happy-dom

import { act, type ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DidPlaceboFakeGroupBlock, PanelDidResultData } from '@/shared/types/report';
import { DIDComponent } from './DIDComponent';

const mocks = vi.hoisted(() => ({
  hookState: null as unknown as {
    permReps: number;
    setPermReps: (value: number) => void;
    rngSeed: number;
    setRngSeed: (value: number) => void;
    display: DidPlaceboFakeGroupBlock | null;
    loading: boolean;
    error: { code: string; incidentId: string | null } | null;
    run: () => Promise<void>;
    canRun: boolean;
  },
  t: vi.fn((key: string, values?: Record<string, unknown>) => {
    if (key === 'did.fakeGroup.methodology') {
      return `methodology:${values?.nPerm}/${values?.nPermValid}/${values?.nTreatedEntities}/${values?.nEntities}`;
    }
    if (key === 'did.fakeGroup.unavailable.insufficient_valid_permutations') {
      return `unavailable:${values?.nPermValid}/${values?.minValidPermutations}`;
    }
    if (key === 'did.fakeGroup.errors.internal_error') return 'Localized internal failure';
    if (key === 'common.incidentId') return 'Incident ID';
    return key;
  }),
}));

vi.mock('@/features/application/stats/statsActions', () => ({
  useDidFakeGroupRi: () => mocks.hookState,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: mocks.t }),
}));

vi.mock('./shared', async () => {
  const React = await import('react');
  const Container = ({ children }: { children?: ReactNode }) => React.createElement('div', null, children);
  return {
    ReportLayout: Container,
    ReportSection: Container,
    PanelFESummaryGrid: () => null,
    ModelSummaryGrid: () => null,
    CoefficientsBlock: () => null,
    HypothesisTestBlock: () => null,
    DidEventStudyChart: () => null,
    OmittedVariablesAlert: () => null,
    formatNum: (value: number) => String(value),
    formatNullableNum: (value: number | null | undefined) => String(value),
  };
});

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

const data = {
  kind: 'panel_did',
  title: 'DID',
  endog_name: 'Y',
  treat_name: 'Treat',
  post_name: 'Post',
  fe_twoway: {
    coefficients: [],
    model_basic_info: {},
    diagnostic_info: undefined,
  },
  fake_group_engine: {
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
  },
} as unknown as PanelDidResultData;

const success: DidPlaceboFakeGroupBlock = {
  available: true,
  observed_coef: 1.75,
  n_perm: 20,
  n_perm_valid: 18,
  min_valid_permutations: 10,
  n_entities: 8,
  n_treated_entities: 3,
  p_value_ri: 0.2,
  perm_coef_mean: 0.1,
  perm_coef_std: 0.3,
};

function hookState(overrides: Partial<typeof mocks.hookState> = {}): typeof mocks.hookState {
  return {
    permReps: 399,
    setPermReps: vi.fn(),
    rngSeed: 42,
    setRngSeed: vi.fn(),
    display: null,
    loading: false,
    error: null,
    run: vi.fn(async () => undefined),
    canRun: true,
    ...overrides,
  };
}

describe('DIDComponent fake-group result', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    mocks.t.mockClear();
    mocks.hookState = hookState();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  function render() {
    act(() => root.render(<DIDComponent data={data} />));
  }

  it('builds successful methodology text from structured numeric fields', () => {
    mocks.hookState = hookState({ display: success });

    render();

    expect(container.textContent).toContain('methodology:20/18/3/8');
    expect(mocks.t).toHaveBeenCalledWith('did.fakeGroup.methodology', {
      nPerm: 20,
      nPermValid: 18,
      nTreatedEntities: 3,
      nEntities: 8,
    });
  });

  it('localizes expected statistical unavailability by its stable code', () => {
    mocks.hookState = hookState({
      display: {
        available: false,
        unavailableCode: 'insufficient_valid_permutations',
        n_perm: 9,
        n_perm_valid: 7,
        min_valid_permutations: 10,
        n_entities: 8,
        n_treated_entities: 3,
      },
    });

    render();

    expect(container.textContent).toContain('unavailable:7/10');
    expect(mocks.t).toHaveBeenCalledWith(
      'did.fakeGroup.unavailable.insufficient_valid_permutations',
      {
        nPerm: 9,
        nPermValid: 7,
        minValidPermutations: 10,
        nTreatedEntities: 3,
        nEntities: 8,
      },
    );
  });

  it('localizes command errors by code and displays an incident ID when present', () => {
    mocks.hookState = hookState({
      error: { code: 'internal_error', incidentId: 'incident-did-42' },
    });

    render();

    expect(container.textContent).toContain('[internal_error] Localized internal failure');
    expect(container.textContent).toContain('Incident ID: incident-did-42');
  });
});
