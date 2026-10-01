import { useEffect, useMemo, useRef, useState } from "react";
import {
  captureProjectReadContext,
  useProjectIOStore,
} from "@/features/application/project/projectIOStore";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { useResourceStore } from "@/features/core/resource";
import { portAddressKey } from "@/features/domain/editorProjection/portAddressKey";
import { getConnectionCandidates } from "@/services/nodeSystem/connectionCandidatesService";
import type {
  ConnectionDecision,
  ConnectionIntent,
} from "@/shared/types/domain/connectionCandidates";
import type { PortAddressDto } from "@/shared/types/domain/editorProjection";
import { freezePublishedValue } from "@/shared/types/deepReadonly";

const EMPTY: Readonly<Partial<Record<string, ConnectionDecision>>> = Object.freeze({});

export function useConnectionCandidates({
  graphPath,
  sourcePort,
  intent,
  enabled,
}: {
  graphPath: string;
  sourcePort: PortAddressDto | null;
  intent: ConnectionIntent;
  enabled: boolean;
}) {
  const active = enabled && sourcePort !== null;
  // Idle canvases do not consume candidate invalidation signals. Enabling a gesture
  // reads the current owners before constructing its request identity.
  const projectInstanceId = useProjectIOStore((state) => (active ? state.projectInstanceId : null));
  const version = useGraphProjectionStore((state) =>
    active ? state.sessions[graphPath]?.version : undefined,
  );
  const semanticInputHash = useGraphProjectionStore((state) =>
    active ? state.sessions[graphPath]?.semanticInputHash : undefined,
  );
  const publicationRevision = useResourceStore((state) => (active ? state.indexRevision : null));
  const sourceKey = sourcePort ? portAddressKey(sourcePort) : null;
  const key =
    projectInstanceId && version && semanticInputHash && sourceKey
      ? JSON.stringify([
          projectInstanceId,
          graphPath,
          version.sessionId,
          version.revision,
          semanticInputHash,
          publicationRevision,
          sourceKey,
          intent,
        ])
      : null;
  const [state, setState] = useState<{
    key: string;
    status: "loading" | "ready" | "error";
    decisions: Readonly<Partial<Record<string, ConnectionDecision>>>;
    isCurrent(): boolean;
  } | null>(null);
  const inFlight = useRef<Promise<void> | null>(null);

  useEffect(() => {
    if (
      !enabled ||
      !key ||
      !projectInstanceId ||
      !version ||
      !sourcePort ||
      (state?.key === key && state.status === "ready" && state.isCurrent())
    )
      return;
    const context = captureProjectReadContext(projectInstanceId);
    if (!context) return;
    const isCurrent = () => {
      const session = useGraphProjectionStore.getState().sessions[graphPath];
      return (
        context.isCurrent() &&
        session?.version.sessionId === version.sessionId &&
        session.version.revision === version.revision &&
        session.semanticInputHash === semanticInputHash &&
        useResourceStore.getState().indexRevision === publicationRevision
      );
    };
    let current = true;
    setState({ key, status: "loading", decisions: EMPTY, isCurrent });
    const read = () =>
      getConnectionCandidates({ projectInstanceId, graphPath, version, sourcePort, intent })
        .then((projection) => {
          if (!current || !isCurrent()) return;
          if (projection.semanticInputHash !== semanticInputHash)
            throw new Error("Stale connection candidates");
          setState({
            key,
            status: "ready",
            isCurrent,
            decisions: freezePublishedValue(
              Object.fromEntries(
                projection.candidates.map((candidate) => [
                  portAddressKey(candidate.port),
                  candidate.decision,
                ]),
              ),
            ),
          });
        })
        .catch(() => {
          if (current && isCurrent())
            setState({ key, status: "error", decisions: EMPTY, isCurrent });
        });
    const start = () => {
      if (!current || !isCurrent()) return;
      const pending = read();
      inFlight.current = pending;
      void pending.then(() => {
        if (inFlight.current === pending) inFlight.current = null;
      });
    };
    // One backend query per consumer at a time; superseded gestures waiting on
    // that query are discarded before they can start another expensive read.
    if (inFlight.current) void inFlight.current.then(start);
    else start();
    return () => {
      current = false;
    };
  }, [enabled, key]);

  // Compare during render, before effects run: an old response must never remain
  // actionable for even one render of a different graph revision or gesture.
  return useMemo(() => {
    const current = enabled && state?.key === key && state.isCurrent() ? state : null;
    return {
      status: !enabled || !key ? ("idle" as const) : (current?.status ?? ("loading" as const)),
      decisions: current?.decisions ?? EMPTY,
      decisionFor: (sourceId: string, currentIntent: ConnectionIntent, targetId: string) =>
        sourceId === sourceKey && currentIntent === intent && current?.isCurrent()
          ? current.decisions[targetId]
          : undefined,
    };
  }, [enabled, key, sourceKey, intent, state]);
}
