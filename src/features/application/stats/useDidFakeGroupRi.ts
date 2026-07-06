import { useCallback, useState } from 'react';
import { PanelDidService } from '@/features/application/stats/statsActions';
import type { DidPlaceboFakeGroupBlock, PanelDidResultData } from '@/views/InfoView/shared/types';

export function useDidFakeGroupRi(
  fakeGroupEngine: PanelDidResultData['fake_group_engine'],
  initialResult: DidPlaceboFakeGroupBlock | null | undefined,
) {
  const [permReps, setPermReps] = useState(399);
  const [rngSeed, setRngSeed] = useState(42);
  const [result, setResult] = useState<DidPlaceboFakeGroupBlock | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const display = result ?? initialResult ?? null;

  const run = useCallback(async () => {
    if (!fakeGroupEngine) return;
    setError(null);
    setLoading(true);
    try {
      const n_perm = Math.max(1, Math.min(2000, Math.floor(permReps) || 399));
      const res = await PanelDidService.computeFakeGroupRi<
        typeof fakeGroupEngine & { n_perm: number; rng_seed: number },
        DidPlaceboFakeGroupBlock
      >({
        ...fakeGroupEngine,
        n_perm,
        rng_seed: Number.isFinite(rngSeed) ? Math.max(0, Math.floor(Number(rngSeed))) : 42,
      });
      setResult(res);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
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
