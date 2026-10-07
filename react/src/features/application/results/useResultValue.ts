import { useCallback, useEffect, useRef, useSyncExternalStore, useState } from "react";

import type { ErrorReference } from "@/features/application/errorReference";
import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type { ResultValue } from "./types";
import type { ResultReference } from "@/shared/types/domain/result";
import type {
  ResultQueryCoordinator,
  ResultQueryReadCapability,
  ResultQueryOutcome,
} from "./resultQueryCoordinator";
import { resultQueryCoordinator, resultQueryRead } from "./runtime";

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
  reference: ResultReference | null,
  dependencies: ResultValueHookDependencies = {
    coordinator: resultQueryCoordinator,
    read: resultQueryRead,
  },
): ResultValueQueryState {
  const [loading, setLoading] = useState(false);
  const generation = useRef(0);
  const value = useSyncExternalStore(
    dependencies.read.subscribe,
    () => (reference === null ? null : dependencies.read.getValue(reference)),
    () => (reference === null ? null : dependencies.read.getValue(reference)),
  );
  const readError = () =>
    reference === null ? null : dependencies.read.getFailure({ kind: "value", ...reference });
  const error = useSyncExternalStore(dependencies.read.subscribe, readError, readError);

  const reload = useCallback(async (): Promise<ResultQueryOutcome> => {
    if (reference === null) return { status: "notReady" };
    const current = ++generation.current;
    setLoading(true);
    try {
      return await dependencies.coordinator.loadValue(reference);
    } finally {
      if (generation.current === current) setLoading(false);
    }
  }, [dependencies.coordinator, reference?.executionSessionId, reference?.resultId]);

  useEffect(() => {
    if (reference === null) {
      setLoading(false);
      return;
    }

    const releasePayload = dependencies.coordinator.retainPayload(reference);
    void reload();

    return () => {
      generation.current += 1;
      releasePayload();
    };
  }, [dependencies.coordinator, reference?.executionSessionId, reference?.resultId, reload]);

  return { value, loading, error, reload };
}
