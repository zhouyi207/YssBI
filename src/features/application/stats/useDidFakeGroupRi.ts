import { useCallback, useMemo, useState } from 'react';
import { PanelDidService } from '@/features/application/stats/statsActions';
import { toErrorReference, type ErrorReference } from '@/features/application/errorReference';
import {
  parseDidPlaceboFakeGroupBlock,
  type DidPlaceboFakeGroupBlock,
  type PanelDidResultData,
} from '@/shared/types/report';

const INVALID_RESPONSE_ERROR: ErrorReference = {
  code: 'did_fake_group_invalid_response',
  incidentId: null,
};
const INVALID_INITIAL_RESULT_ERROR: ErrorReference = {
  code: 'did_fake_group_invalid_initial_result',
  incidentId: null,
};

export function useDidFakeGroupRi(
  fakeGroupEngine: PanelDidResultData['fake_group_engine'],
  initialResult: DidPlaceboFakeGroupBlock | null | undefined,
) {
  const [permReps, setPermReps] = useState(399);
  const [rngSeed, setRngSeed] = useState(42);
  const [result, setResult] = useState<DidPlaceboFakeGroupBlock | null>(null);
  const [loading, setLoading] = useState(false);
  const [requestError, setRequestError] = useState<ErrorReference | null>(null);

  const parsedInitialResult = useMemo(
    () => initialResult == null ? null : parseDidPlaceboFakeGroupBlock(initialResult),
    [initialResult],
  );
  const display = result ?? parsedInitialResult;
  const initialResultError = initialResult != null && parsedInitialResult === null && result === null
    ? INVALID_INITIAL_RESULT_ERROR
    : null;
  const error = requestError ?? initialResultError;

  const run = useCallback(async () => {
    if (!fakeGroupEngine) return;
    setRequestError(null);
    setLoading(true);
    try {
      const n_perm = Math.max(1, Math.min(2000, Math.floor(permReps) || 399));
      const raw = await PanelDidService.computeFakeGroupRi<
        typeof fakeGroupEngine & { n_perm: number; rng_seed: number },
        unknown
      >({
        ...fakeGroupEngine,
        n_perm,
        rng_seed: Number.isFinite(rngSeed) ? Math.max(0, Math.floor(Number(rngSeed))) : 42,
      });
      const parsed = parseDidPlaceboFakeGroupBlock(raw);
      if (!parsed) {
        setRequestError(INVALID_RESPONSE_ERROR);
        return;
      }
      setResult(parsed);
    } catch (caught) {
      setRequestError(toErrorReference(caught, 'did_fake_group_request_failed'));
    } finally {
      setLoading(false);
    }
  }, [fakeGroupEngine, permReps, rngSeed]);

  return {
    permReps,
    setPermReps,
    rngSeed,
    setRngSeed,
    display,
    loading,
    error,
    run,
    canRun: Boolean(fakeGroupEngine),
  };
}
