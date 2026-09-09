import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import {
  captureProjectLifecycleState,
  isProjectLifecycleStateCurrent,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { getActivityPanelDocument } from "@/services/workbench/activityPanelService";
import { DEFAULT_LANGUAGE } from "@/shared/types/settings";
import type { ActivityPanelId, ActivityPanelSnapshot } from "@/shared/types/domain/activityPanel";
import { useActivityPanelExpansion } from "./useActivityPanelExpansion";
import { formatInlineUserError } from "../userErrorSummary";

/** Existing projections only invalidate this query; Rust supplies snapshots and row operations. */
export function useActivityPanelDocument(panelId: ActivityPanelId, invalidation?: unknown) {
  const { i18n, t } = useTranslation();
  const expansion = useActivityPanelExpansion(panelId);
  const scoped = panelId === "project" || panelId === "nodes";
  const projectInstanceId = useProjectIOStore((state) => (scoped ? state.projectInstanceId : null));
  const resources = useResourceStore((state) => (scoped ? state.resources : null));
  const locale = i18n.resolvedLanguage || i18n.language || DEFAULT_LANGUAGE;
  const epoch = scoped ? captureProjectLifecycleState().epoch : 0;
  const binding = useMemo<{ refresh: (() => void) | null }>(
    () => ({ refresh: null }),
    [panelId, projectInstanceId, locale, epoch],
  );
  const [result, setResult] = useState<{
    binding: typeof binding;
    snapshot: ActivityPanelSnapshot | null;
    error: unknown;
  } | null>(null);

  useEffect(() => {
    const identity = captureProjectLifecycleState();
    let closed = false;
    let pending = false;
    let requested = false;
    let snapshot: ActivityPanelSnapshot | null = null;
    const ownsBinding = () => !closed && (!scoped || isProjectLifecycleStateCurrent(identity));
    const refresh = async () => {
      requested = true;
      if (pending) return;
      pending = true;
      try {
        // Serialize within a binding and coalesce invalidations, retaining the last delivered cursor.
        while (requested && ownsBinding()) {
          requested = false;
          try {
            const next = await getActivityPanelDocument(
              panelId,
              projectInstanceId ? { projectInstanceId } : null,
              locale,
              snapshot,
            );
            if (!ownsBinding()) return;
            snapshot = next;
            if (!requested)
              setResult((current) =>
                current?.binding === binding && current.snapshot === next && !current.error
                  ? current
                  : { binding, snapshot: next, error: null },
              );
          } catch (error: unknown) {
            if (ownsBinding() && !requested) setResult({ binding, snapshot, error });
          }
        }
      } finally {
        pending = false;
      }
    };
    binding.refresh = () => {
      void refresh();
    };
    return () => {
      closed = true;
      binding.refresh = null;
    };
  }, [binding, panelId, projectInstanceId, locale, scoped]);

  useEffect(() => {
    binding.refresh?.();
  }, [binding, resources, invalidation]);
  const current = result?.binding === binding ? result : null;
  return {
    document: current?.snapshot?.document ?? null,
    error: current?.error ? formatInlineUserError(current.error, t) : null,
    refresh: () => binding.refresh?.(),
    ...expansion,
  };
}
