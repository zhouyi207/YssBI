import { useEffect, useSyncExternalStore, useState } from "react";

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
  const value = useSyncExternalStore(
    dependencies.read.subscribe,
    () => (reference === null ? null : dependencies.read.getValue(reference)),
    () => (reference === null ? null : dependencies.read.getValue(reference)),
  );
  const error =
    reference === null ? null : dependencies.read.getFailure({ kind: "value", ...reference });

  const reload = async (): Promise<ResultQueryOutcome> => {
    if (reference === null) return { status: "notReady" };
    return dependencies.coordinator.loadValue(reference);
  };

  useEffect(() => {
    let mounted = true;
    if (reference === null) {
      setLoading(false);
      return () => {
        mounted = false;
      };
    }

    const releasePayload = dependencies.coordinator.retainPayload(reference);
    setLoading(true);
    void dependencies.coordinator
      .loadValue(reference)
      .then(() => {
        if (mounted) setLoading(false);
      })
      .catch(() => {
        if (mounted) setLoading(false);
      });

    return () => {
      mounted = false;
      releasePayload();
    };
  }, [dependencies.coordinator, reference?.executionSessionId, reference?.resultId]);

  return { value, loading, error, reload };
}
