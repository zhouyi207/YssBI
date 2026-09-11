import { ResultService } from "@/services/result/resultService";
import {
  resultReferenceKey,
  type ResultLease,
  type ResultReference,
} from "@/shared/types/domain/result";
import { toErrorReference } from "@/features/application/errorReference";
import { logger } from "@/features/application/observability/appLogger";

type LeasePort = Pick<typeof ResultService, "retain" | "release" | "reconcileLeases">;

/** Serializes ownership changes, not data reads or statistical work. Dockview owns live panels. */
export function createResultLeaseController(
  port: LeasePort,
  reportFailure: (error: unknown) => void,
) {
  let tail: Promise<unknown> = Promise.resolve();
  let readPanels: (() => readonly string[] | null) | undefined;
  let scheduled = false;
  let acknowledged: string | undefined;
  const pending = new Set<string>();
  const enqueue = <T>(run: () => Promise<T>): Promise<T> => {
    const next = tail.then(run);
    tail = next.catch(() => undefined);
    return next;
  };
  const reconcile = () => {
    if (scheduled) return;
    scheduled = true;
    void enqueue(async () => {
      scheduled = false;
      const panels = readPanels?.();
      if (!panels) return;
      const active = [...new Set([...panels, ...pending])].sort();
      const key = active.join(",");
      if (key === acknowledged) return;
      await port.reconcileLeases(active);
      acknowledged = key;
    }).catch(reportFailure);
  };
  return {
    bind(read: () => readonly string[] | null) {
      readPanels = read;
      acknowledged = undefined;
      reconcile();
      return () => {
        if (readPanels === read) readPanels = undefined;
      };
    },
    reconcile,
    async acquire(reference: ResultReference, handoff?: string): Promise<ResultLease> {
      const leaseId = crypto.randomUUID();
      pending.add(leaseId);
      try {
        const held = await enqueue(() => port.retain(reference, leaseId, handoff));
        if (
          held.leaseId !== leaseId ||
          resultReferenceKey(held.descriptor) !== resultReferenceKey(reference)
        )
          throw new Error("Mismatched result lease");
        return held;
      } catch (error) {
        await enqueue(() => port.release(leaseId)).catch(reportFailure);
        pending.delete(leaseId);
        reconcile();
        throw error;
      }
    },
    async finish(leaseId: string, installed: boolean) {
      if (!installed) await enqueue(() => port.release(leaseId)).catch(reportFailure);
      pending.delete(leaseId);
      reconcile();
    },
    whenIdle: async () => {
      while (true) {
        const current = tail;
        await current;
        if (current === tail) return;
      }
    },
  };
}

export const resultLeases = createResultLeaseController(ResultService, (error) => {
  const failure = toErrorReference(error, "result_lease_failed");
  logger.app.error(
    `Result lease failed code=${failure.code} incidentId=${failure.incidentId ?? "none"}`,
    "ResultLease",
  );
});
