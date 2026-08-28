import { useEffect, useSyncExternalStore, useState } from 'react';

import type { ErrorReference } from '@/services/ipc';
import type { DeepReadonly } from '@/features/core/projection/deepReadonly';
import type { ResultValue } from '@/shared/types/dto/result';
import type {
  ResultQueryCoordinator,
  ResultQueryReadCapability,
  ResultQueryOutcome,
} from './resultQueryCoordinator';
import { resultQueryCoordinator, resultQueryRead } from './runtime';

export interface ResultValueHookDependencies {
  readonly coordinator: ResultQueryCoordinator;
  readonly read: ResultQueryReadCapability;
}

export interface ResultValueQueryState {
  readonly value: DeepReadonly<ResultValue | null>;
  readonly loading: boolean;
  readonly error: DeepReadonly<ErrorReference> | null;
  readonly reload: () => Promise<ResultQueryOutcome>;
}

export function useResultValue(
  resultId: string | null,
  dependencies: ResultValueHookDependencies = {
    coordinator: resultQueryCoordinator,
    read: resultQueryRead,
  },
): ResultValueQueryState {
  const [loading, setLoading] = useState(false);
  const value = useSyncExternalStore(
    dependencies.read.subscribe,
    () => resultId === null ? null : dependencies.read.getValue(resultId),
    () => resultId === null ? null : dependencies.read.getValue(resultId),
  );
  const error = resultId === null
    ? null
    : dependencies.read.getFailure({ kind: 'value', resultId });

  const reload = async (): Promise<ResultQueryOutcome> => {
    if (resultId === null) return { status: 'notReady' };
    return dependencies.coordinator.loadValue({ resultId });
  };

  useEffect(() => {
    let mounted = true;
    if (resultId === null) {
      setLoading(false);
      return () => {
        mounted = false;
      };
    }

    setLoading(true);
    void dependencies.coordinator.loadValue({ resultId }).then(() => {
      if (mounted) setLoading(false);
    }).catch(() => {
      if (mounted) setLoading(false);
    });

    return () => {
      mounted = false;
    };
  }, [dependencies.coordinator, resultId]);

  return { value, loading, error, reload };
}
