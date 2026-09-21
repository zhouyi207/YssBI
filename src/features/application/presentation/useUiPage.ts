import { useEffect, useRef, useState } from "react";
import {
  activateUiElement,
  readUiPage,
  subscribeUi,
  updateUiPage,
} from "@/services/workbench/presentationService";
import { installUiUpdate } from "@/shared/types/domain/uiPresentation";
import { resultReferenceKey, type ResultReference } from "@/shared/types/domain/result";
import type { UiAction, UiPage, UiUpdate } from "@/shared/types/domain/uiPresentation";
import type { ErrorReference } from "@/shared/types/domain/errorReference";
import {
  captureProjectLifecycleState,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { useProjectProjection } from "@/features/application/project/projectProjection";
import { toErrorReference } from "@/features/application/errorReference";
import { initializeProjectForCurrentWindow } from "@/features/application/project/projectRuntime";

function errorReference(error: unknown): ErrorReference {
  return toErrorReference(error, "ui_spec_invalid");
}

export function useUiPage(source: ResultReference) {
  const { projectInstanceId } = useProjectProjection();
  const [page, setPage] = useState<UiPage | null>(null);
  const [error, setError] = useState<ErrorReference | null>(null);
  const [busy, setBusy] = useState(false);
  const [connection, setConnection] = useState(0);
  const binding = useRef<{
    page: UiPage | null;
    current(): boolean;
    reload(): Promise<void>;
    install(update: UiUpdate): void;
    working: boolean;
  } | null>(null);

  useEffect(() => {
    if (projectInstanceId) return;
    let active = true;
    // Standalone result windows use the same project lifecycle entrance as other windows.
    void initializeProjectForCurrentWindow().catch((error) => {
      if (active) setError(errorReference(error));
    });
    return () => {
      active = false;
    };
  }, [projectInstanceId]);

  useEffect(() => {
    setPage(null);
    setError(null);
    setBusy(false);
    if (!projectInstanceId) return;
    const identity = captureProjectLifecycleState();
    if (identity.projectInstanceId !== projectInstanceId) return;
    let closed = false;
    let loading: Promise<void> | null = null;
    let reloadAfterFlight = false;
    const entry = {
      page: null as UiPage | null,
      working: false,
      current: () =>
        !closed && isCurrentProjectIdentity({ projectInstanceId, epoch: identity.epoch }),
      install: (update: UiUpdate) => {
        if (!entry.current()) return;
        const next = installUiUpdate(entry.page, update, source);
        entry.page = next;
        setPage(next);
        setError(null);
      },
      reload: (fresh = false): Promise<void> => {
        if (loading) {
          reloadAfterFlight ||= fresh;
          return loading;
        }
        loading = readUiPage(projectInstanceId, source)
          .then((page) => entry.install({ kind: "snapshot", page }))
          .catch((error) => {
            if (entry.current()) setError(errorReference(error));
          })
          .finally(() => {
            loading = null;
            if (reloadAfterFlight && entry.current()) {
              reloadAfterFlight = false;
              void entry.reload();
            }
          });
        return loading;
      },
    };
    binding.current = entry;
    // Subscribe before the initial snapshot so updates cannot fall between query and attachment.
    const subscription = subscribeUi(
      projectInstanceId,
      false,
      (event) => {
        if (!entry.current()) return;
        if (event.kind === "sessionChanged") {
          closed = true;
          setConnection((value) => value + 1);
          return;
        }
        if (event.kind === "resync") {
          void entry.reload(true);
          return;
        }
        if (event.kind !== "update") return;
        const incoming =
          event.update.kind === "snapshot" ? event.update.page.source : event.update.source;
        if (resultReferenceKey(incoming) !== resultReferenceKey(source)) return;
        try {
          entry.install(event.update);
        } catch {
          void entry.reload(true);
        }
      },
      (error) => {
        if (entry.current()) {
          setError(errorReference(error));
          void entry.reload(true);
        }
      },
    )
      .then(async (close) => {
        if (!entry.current()) {
          await close();
          return () => Promise.resolve();
        }
        if (!entry.page && !loading) await entry.reload();
        return close;
      })
      .catch((error) => {
        if (entry.current()) setError(errorReference(error));
        return () => Promise.resolve();
      });
    return () => {
      closed = true;
      if (binding.current === entry) binding.current = null;
      void subscription.then((close) => close()).catch(() => {});
    };
  }, [projectInstanceId, source.executionSessionId, source.resultId, connection]);

  const run = async (operation: (page: UiPage) => Promise<UiUpdate | null>) => {
    const entry = binding.current;
    if (!entry?.page || !entry.current() || entry.working) return false;
    entry.working = true;
    setBusy(true);
    try {
      const update = await operation(entry.page);
      if (!entry.current()) return false;
      if (update) entry.install(update);
      return true;
    } catch (error) {
      // A missing reply is not permission to repeat a possibly committed action.
      if (entry.current()) {
        await entry.reload();
        if (entry.current()) setError(errorReference(error));
      }
      return false;
    } finally {
      entry.working = false;
      if (entry.current()) setBusy(false);
    }
  };
  return {
    page,
    error,
    busy,
    reload: () => binding.current?.reload(),
    act: (action: UiAction) =>
      run((page) => updateUiPage(projectInstanceId!, source, page.revision, action)),
    activate: (id: string) =>
      run(async (page) => {
        await activateUiElement(projectInstanceId!, source, page.revision, id);
        return null;
      }),
  };
}
