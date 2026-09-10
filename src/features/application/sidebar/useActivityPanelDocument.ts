import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import {
  captureProjectLifecycleState,
  isProjectLifecycleStateCurrent,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { useSidebarStore, type ActivityPanelBinding } from "@/features/core/sidebar/sidebarStore";
import { getActivityPanelDocument } from "@/services/workbench/activityPanelService";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";
import { DEFAULT_LANGUAGE } from "@/shared/types/settings";
import type { ActivityPanelId } from "@/shared/types/domain/activityPanel";
import { useActivityPanelExpansion } from "./useActivityPanelExpansion";
import { toErrorReference } from "../errorReference";

// Requests are shared by mounted consumers of the same cached binding.
const requests = new Map<ActivityPanelBinding, { again: boolean; promise: Promise<void> }>();
function requestStandalone(binding: ActivityPanelBinding): Promise<void> {
  const pending = requests.get(binding);
  if (pending) {
    pending.again = true;
    return pending.promise;
  }
  const owns = () => useSidebarStore.getState().panels[binding.panelId]?.binding === binding;
  const entry = { again: true, promise: Promise.resolve() };
  requests.set(binding, entry);
  useSidebarStore.getState().startPanelRequest(binding);
  entry.promise = (async () => {
    let previous = useSidebarStore.getState().panels[binding.panelId]?.snapshot ?? null;
    try {
      while (entry.again && owns()) {
        entry.again = false;
        try {
          previous = await getActivityPanelDocument(
            binding.panelId,
            binding.projectInstanceId ? { projectInstanceId: binding.projectInstanceId } : null,
            binding.locale,
            previous,
          );
          if (owns() && !entry.again)
            useSidebarStore.getState().publishPanels([{ binding, snapshot: previous }]);
        } catch (error) {
          if (owns() && !entry.again)
            useSidebarStore
              .getState()
              .failPanelRequest(binding, toErrorReference(error, "activity_panel_sync_failed"));
        }
      }
    } finally {
      requests.delete(binding);
    }
  })();
  return entry.promise;
}
function refreshPanel(binding: ActivityPanelBinding): void {
  if ((binding.panelId === "project" || binding.panelId === "nodes") && binding.projectInstanceId) {
    if (!isProjectLifecycleStateCurrent(binding)) return;
    useSidebarStore.getState().startPanelRequest(binding);
    void projectPublicationCoordinator.refreshIndex().catch((error) => {
      if (!useSidebarStore.getState().panels[binding.panelId]?.error)
        useSidebarStore
          .getState()
          .failPanelRequest(binding, toErrorReference(error, "activity_panel_sync_failed"));
    });
  } else {
    void requestStandalone(binding);
  }
}

/** All content is a cached Rust document; expansion is local UI state. */
export function useActivityPanelDocument(panelId: ActivityPanelId, invalidation?: unknown) {
  const { i18n, t } = useTranslation();
  const scoped = panelId === "project" || panelId === "nodes";
  const installedProject = useProjectIOStore((state) => (scoped ? state.projectInstanceId : null));
  const lifecycle = captureProjectLifecycleState();
  const projectInstanceId =
    scoped && installedProject === lifecycle.projectInstanceId ? installedProject : null;
  const locale = i18n.resolvedLanguage || i18n.language || DEFAULT_LANGUAGE;
  const epoch = scoped ? lifecycle.epoch : 0;
  const entry = useSidebarStore((state) => state.panels[panelId] ?? null);
  const matchesScope =
    entry?.binding.projectInstanceId === projectInstanceId &&
    entry.binding.locale === locale &&
    entry.binding.epoch === epoch;
  const current = matchesScope ? entry : null;
  useEffect(() => {
    const binding = useSidebarStore
      .getState()
      .bindPanel({ panelId, projectInstanceId, locale, epoch });
    const entry = useSidebarStore.getState().panels[panelId]!;
    if ((!entry.snapshot && !entry.loading) || invalidation !== undefined) refreshPanel(binding);
  }, [panelId, projectInstanceId, locale, epoch, invalidation]);
  return {
    document: current?.snapshot?.document ?? null,
    error: current?.error
      ? `${t("common.error")} [${current.error.code}]${current.error.incidentId ? ` · ${t("common.incidentId")}: ${current.error.incidentId}` : ""}`
      : null,
    loading: current?.loading ?? false,
    refresh: () =>
      refreshPanel(
        useSidebarStore.getState().bindPanel({ panelId, projectInstanceId, locale, epoch }),
      ),
    ...useActivityPanelExpansion(panelId),
  };
}
