import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import { loadActivatedProject } from "@/features/application/project/projectHydration";
import {
  applyCleanupProgressEvent,
  applyScanProgressEvent,
  markProjectPickerProgressDone,
  projectPickerProgressInitial,
  projectPickerScanFolderTitle,
  runWithProjectPickerProgress,
  updateOpenProjectProgressStage,
} from "@/features/application/project/projectPickerProgress";
import { ProjectService, isPickerTaskCancelledError } from "@/services/project/projectService";
import { revealPath } from "@/services/platform/opener";
import { openPathDialog } from "@/services/platform/pathDialog";
import type { ProjectRecordRow } from "@/shared/types/domain/project";
import {
  ProjectPickerOperationError,
  isProjectPickerStaleError,
  projectPickerErrorPresentation,
  projectPickerRecoveryPresentation,
  type ProjectPickerLifecycleActionOutcome,
  type ProjectPickerPageActionOutcome,
  type ProjectPickerPageIssue,
  type ProjectPickerPageOperation,
} from "./projectPickerOutcomes";

import {
  ProjectLifecycleProtocolError,
  applyProjectLifecycleReceipt,
  claimProjectLifecycleInitiatorSettlement,
  recoverProjectLifecycleDirectFailure,
  registerPendingProjectLifecycleOperation,
  type PendingProjectLifecycleOperation,
  type ProjectLifecycleReceiptSettlement,
} from "@/features/application/projectLifecycleReceipt";
import { createProjectLifecycleReceiptDependencies } from "@/features/application/projectLifecycleReceiptDependencies";

export interface ManagedProject {
  id: string;
  name: string;
  path: string;
  lastOpenedAt: string;
  isFavorite?: boolean;
}

type BusyState = "idle" | "new" | "registry" | "delete" | ProjectPickerPageOperation["operation"];

interface PickerOperation {
  readonly finished: Promise<void>;
  isCurrent(): boolean;
  finish(): void;
}

class ProjectPickerTaskCancelled extends Error {}

function pathFileName(path: string): string {
  const normalized = path.replace(/\\/g, "/");
  const parts = normalized.split("/").filter(Boolean);
  const file = parts.length > 0 ? parts[parts.length - 1] : path;
  if (file.toLowerCase() === "metadata.yssbi") {
    const parent = parts.length > 1 ? parts[parts.length - 2] : undefined;
    return parent || file.replace(/\.[^.]+$/, "") || file;
  }
  return file.replace(/\.[^.]+$/, "") || file;
}

function rowToManagedProject(row: ProjectRecordRow): ManagedProject {
  return {
    id: row.id,
    name: row.name,
    path: row.path,
    lastOpenedAt: row.lastOpenedAt ?? row.createdAt,
    isFavorite: row.isFavorite,
  };
}

async function listManagedProjects(): Promise<ManagedProject[]> {
  const rows = await ProjectService.listRegisteredProjects();
  return rows.map(rowToManagedProject);
}

function managedProjectsFromSettlement(
  settlement: ProjectLifecycleReceiptSettlement,
): ManagedProject[] {
  if (!settlement.registryProjects) {
    throw new Error("Lifecycle settlement omitted registry projection");
  }
  return settlement.registryProjects.map(rowToManagedProject);
}

export function useProjectPicker() {
  const navigate = useNavigate();
  const currentPath = useProjectIOStore((state) => state.currentPath);
  const [projects, setProjects] = useState<ManagedProject[]>([]);
  const [busy, setBusy] = useState<BusyState>("idle");
  const [pageIssue, setPageIssue] = useState<ProjectPickerPageIssue | null>(null);
  const mounted = useRef(false);
  const activeOperation = useRef<PickerOperation | null>(null);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const beginOperation = useCallback((kind: Exclude<BusyState, "idle">): PickerOperation | null => {
    if (!mounted.current) return null;
    if (activeOperation.current) throw new ProjectPickerOperationError("project_picker_busy");
    let complete!: () => void;
    const finished = new Promise<void>((resolve) => {
      complete = resolve;
    });
    const operation: PickerOperation = {
      finished,
      isCurrent: () => mounted.current && activeOperation.current === operation,
      finish: () => {
        if (activeOperation.current === operation) {
          activeOperation.current = null;
          if (mounted.current) setBusy("idle");
        }
        complete();
      },
    };
    activeOperation.current = operation;
    setBusy(kind);
    return operation;
  }, []);

  const publishPageIssue = useCallback(
    (
      issue: ProjectPickerPageIssue,
      operation: PickerOperation | null,
    ): ProjectPickerPageActionOutcome => {
      if (operation && !operation.isCurrent()) return { status: "stale" };
      if (operation) setPageIssue(issue);
      return { status: "issue", issue };
    },
    [],
  );

  const dismissPageIssue = useCallback(() => {
    setPageIssue(null);
  }, []);

  const handlePickerTaskCancelled = useCallback(
    async (operation: PickerOperation): Promise<ProjectPickerPageActionOutcome> => {
      if (!operation.isCurrent()) return { status: "stale" };
      try {
        const projects = await listManagedProjects();
        if (!operation.isCurrent()) return { status: "stale" };
        setProjects(projects);
        return { status: "cancelled" };
      } catch (error) {
        return publishPageIssue(
          {
            kind: "failure",
            operation: "refresh",
            error: projectPickerErrorPresentation(error),
          },
          operation,
        );
      }
    },
    [publishPageIssue],
  );

  const runPageOperation = useCallback(
    async (
      context: ProjectPickerPageOperation,
      run: (operation: PickerOperation) => Promise<ProjectPickerPageActionOutcome>,
    ): Promise<ProjectPickerPageActionOutcome> => {
      let operation: PickerOperation | null = null;
      try {
        operation = beginOperation(context.operation);
        if (!operation) return { status: "stale" };
        setPageIssue(null);
        return await run(operation);
      } catch (error) {
        if (
          operation &&
          (context.operation === "scan" || context.operation === "cleanup") &&
          (error instanceof ProjectPickerTaskCancelled || isPickerTaskCancelledError(error))
        )
          return await handlePickerTaskCancelled(operation);
        if (isProjectPickerStaleError(error)) return { status: "stale" };
        return publishPageIssue(
          { ...context, kind: "failure", error: projectPickerErrorPresentation(error) },
          operation,
        );
      } finally {
        operation?.finish();
      }
    },
    [beginOperation, handlePickerTaskCancelled, publishPageIssue],
  );

  const refresh = useCallback(
    () =>
      runPageOperation({ operation: "refresh" }, async (operation) => {
        const projects = await listManagedProjects();
        if (!operation.isCurrent()) return { status: "stale" };
        setProjects(projects);
        return { status: "completed" };
      }),
    [runPageOperation],
  );

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    if (!currentPath) return;
    let cancelled = false;
    void (async () => {
      while (activeOperation.current) {
        await activeOperation.current.finished;
        if (cancelled || !mounted.current) return;
      }
      const operation = beginOperation("registry");
      if (!operation) return;
      try {
        const row = await ProjectService.registerProject(pathFileName(currentPath), currentPath);
        if (cancelled || !operation.isCurrent()) return;
        setProjects((previous) => [
          rowToManagedProject(row),
          ...previous.filter((project) => project.id !== row.id),
        ]);
      } catch {
        if (cancelled || !operation.isCurrent()) return;
        try {
          const projects = await listManagedProjects();
          if (!cancelled && operation.isCurrent()) setProjects(projects);
        } catch (error) {
          if (!cancelled)
            publishPageIssue(
              {
                kind: "failure",
                operation: "refresh",
                error: projectPickerErrorPresentation(error),
              },
              operation,
            );
        }
      } finally {
        operation.finish();
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [beginOperation, currentPath, publishPageIssue]);

  const currentProjectId = useMemo(
    () => projects.find((project) => project.path === currentPath)?.id ?? null,
    [currentPath, projects],
  );

  const scanProjectsFromFolder = useCallback(
    () =>
      runPageOperation({ operation: "scan" }, async (operation) => {
        const selection = await openPathDialog({
          directory: true,
          multiple: false,
          title: projectPickerScanFolderTitle(),
        });
        if (!operation.isCurrent()) return { status: "stale" };
        if (!selection.ok) throw new Error(selection.failure.code);
        const directory = selection.value;
        if (!directory) return { status: "cancelled" };
        if (Array.isArray(directory)) return { status: "cancelled" };

        const { result, cancelled } = await runWithProjectPickerProgress(
          {
            initial: projectPickerProgressInitial("scan"),
            onCancel: () => {
              void ProjectService.cancelProjectPickerTask();
            },
          },
          async ({ update, isCancelled }) => {
            try {
              const scanResult = await ProjectService.scanProjectsInDirectory(
                directory,
                (event) => {
                  if (isCancelled()) return;
                  applyScanProgressEvent(event, update);
                },
              );
              if (!isCancelled()) {
                markProjectPickerProgressDone(update);
              }
              return scanResult;
            } catch (error) {
              if (isCancelled() || isPickerTaskCancelledError(error)) {
                throw new ProjectPickerTaskCancelled();
              }
              throw error;
            }
          },
        );

        if (cancelled) return await handlePickerTaskCancelled(operation);

        if (!operation.isCurrent()) return { status: "stale" };
        const projects = await listManagedProjects();
        if (!operation.isCurrent()) return { status: "stale" };
        setProjects(projects);
        if (result.discovered === 0) {
          return publishPageIssue(
            {
              kind: "empty",
              operation: "scan",
              reason: "noneFound",
              found: 0,
            },
            operation,
          );
        }
        if (result.newlyRegistered === 0) {
          return publishPageIssue(
            {
              kind: "empty",
              operation: "scan",
              reason: "alreadyRegistered",
              found: result.discovered,
            },
            operation,
          );
        }
        return { status: "completed" };
      }),
    [runPageOperation, handlePickerTaskCancelled, publishPageIssue],
  );

  const cleanupInvalidProjects = useCallback(
    () =>
      runPageOperation({ operation: "cleanup" }, async (operation) => {
        const { result, cancelled } = await runWithProjectPickerProgress(
          {
            initial: projectPickerProgressInitial("cleanup"),
            onCancel: () => {
              void ProjectService.cancelProjectPickerTask();
            },
          },
          async ({ update, isCancelled }) => {
            try {
              const cleanupResult = await ProjectService.cleanupInvalidRegisteredProjects(
                (event) => {
                  if (isCancelled()) return;
                  applyCleanupProgressEvent(event, update);
                },
              );
              if (!isCancelled()) {
                markProjectPickerProgressDone(update);
              }
              return cleanupResult;
            } catch (error) {
              if (isCancelled() || isPickerTaskCancelledError(error)) {
                throw new ProjectPickerTaskCancelled();
              }
              throw error;
            }
          },
        );

        if (cancelled) return await handlePickerTaskCancelled(operation);

        if (!operation.isCurrent()) return { status: "stale" };
        const projects = await listManagedProjects();
        if (!operation.isCurrent()) return { status: "stale" };
        setProjects(projects);
        if (result.removed === 0) {
          return publishPageIssue(
            {
              kind: "empty",
              operation: "cleanup",
              reason: "noneFound",
            },
            operation,
          );
        }
        return { status: "completed" };
      }),
    [runPageOperation, handlePickerTaskCancelled, publishPageIssue],
  );

  const createProject = useCallback(
    async (name: string, path: string): Promise<ProjectPickerLifecycleActionOutcome> => {
      let operation: PickerOperation | null = null;
      let pending: PendingProjectLifecycleOperation | undefined;
      try {
        operation = beginOperation("new");
        if (!operation) return { status: "stale" };
        pending = registerPendingProjectLifecycleOperation({
          kind: "create",
          expectsActiveProject: false,
        });
        const progress = await runWithProjectPickerProgress(
          {
            initial: projectPickerProgressInitial("create"),
          },
          async ({ update }): Promise<ProjectPickerLifecycleActionOutcome> => {
            let settlement: ProjectLifecycleReceiptSettlement;
            try {
              const result = await ProjectService.createProject(name, path, pending!.operationId);
              if (!pending!.isCurrent()) return { status: "stale" };
              settlement = await applyProjectLifecycleReceipt(
                result,
                "direct",
                createProjectLifecycleReceiptDependencies(),
              );
            } catch (error) {
              if (error instanceof ProjectLifecycleProtocolError && error.zeroEffects) throw error;
              const recovered = await recoverProjectLifecycleDirectFailure(pending!.operationId);
              if (!recovered) throw error;
              settlement = recovered;
            }
            if (settlement.status === "stale" || !pending!.isCurrent()) {
              return { status: "stale" };
            }
            const claimed = claimProjectLifecycleInitiatorSettlement(pending!.operationId);
            if (!claimed) return { status: "stale" };
            if (operation!.isCurrent()) setProjects(managedProjectsFromSettlement(claimed));
            markProjectPickerProgressDone(update);
            if (claimed.result.outcome === "committed" && claimed.result.record) {
              return { status: "committed" };
            }
            return {
              status: "recovery",
              recovery: projectPickerRecoveryPresentation(claimed.result),
            };
          },
        );
        return progress.result;
      } catch (error) {
        if (
          (error instanceof ProjectLifecycleProtocolError && error.zeroEffects) ||
          (pending && !pending.isCurrent()) ||
          isProjectPickerStaleError(error)
        ) {
          return { status: "stale" };
        }
        return {
          status: "failed",
          error: projectPickerErrorPresentation(error),
        };
      } finally {
        operation?.finish();
      }
    },
    [beginOperation],
  );

  const openRecentProject = useCallback(
    (path: string) =>
      runPageOperation({ operation: "open", projectPath: path }, async (operation) => {
        const progress = await runWithProjectPickerProgress(
          {
            initial: projectPickerProgressInitial("open"),
          },
          async ({ update }): Promise<ProjectPickerPageActionOutcome> => {
            const result = await ProjectService.loadProjectToState(path);
            updateOpenProjectProgressStage(update, "loadingData");
            const row = await ProjectService.registerProject(
              pathFileName(result.path),
              result.path,
            );
            const loadReceipt = await loadActivatedProject(result);
            if (!loadReceipt) {
              if (!useProjectIOStore.getState().error) return { status: "stale" };
              throw new ProjectPickerOperationError("project_activation_failed");
            }
            if (!operation.isCurrent()) return { status: "stale" };
            updateOpenProjectProgressStage(update, "preparingEditor");
            setProjects((previous) => [
              rowToManagedProject(row),
              ...previous.filter((project) => project.id !== row.id),
            ]);
            markProjectPickerProgressDone(update);
            navigate("/editor");
            return { status: "completed" };
          },
        );
        return progress.result;
      }),
    [runPageOperation, navigate],
  );

  const importProjectFromDisk = useCallback(
    () =>
      runPageOperation({ operation: "import" }, async (operation) => {
        const selection = await openPathDialog({
          multiple: false,
          filters: [{ name: "YssBI Project", extensions: ["yssbi"] }],
        });
        if (!operation.isCurrent()) return { status: "stale" };
        if (!selection.ok) throw new Error(selection.failure.code);
        const path = selection.value;
        if (!path) return { status: "cancelled" };
        if (Array.isArray(path)) return { status: "cancelled" };

        const row = await ProjectService.registerProject(pathFileName(path), path);
        if (!operation.isCurrent()) return { status: "stale" };
        setProjects((previous) => [
          rowToManagedProject(row),
          ...previous.filter((project) => project.id !== row.id),
        ]);
        return { status: "completed" };
      }),
    [runPageOperation],
  );

  const removeProject = useCallback(
    (id: string) =>
      runPageOperation({ operation: "remove", projectId: id }, async (operation) => {
        await ProjectService.removeRegisteredProject(id);
        if (!operation.isCurrent()) return { status: "stale" };
        setProjects((previous) => previous.filter((project) => project.id !== id));
        return { status: "completed" };
      }),
    [runPageOperation],
  );

  const deleteProjectFiles = useCallback(
    async (id: string): Promise<ProjectPickerLifecycleActionOutcome> => {
      let operation: PickerOperation | null = null;
      let pending: PendingProjectLifecycleOperation | undefined;
      try {
        operation = beginOperation("delete");
        if (!operation) return { status: "stale" };
        const active = id === currentProjectId;
        pending = registerPendingProjectLifecycleOperation({
          kind: "delete",
          expectsActiveProject: active,
        });
        let settlement: ProjectLifecycleReceiptSettlement;
        try {
          const result = await ProjectService.deleteRegisteredProjectFiles(
            id,
            active ? pending.projectInstanceId : null,
            pending.operationId,
          );
          if (!pending.isCurrent()) return { status: "stale" };
          settlement = await applyProjectLifecycleReceipt(
            result,
            "direct",
            createProjectLifecycleReceiptDependencies(),
          );
        } catch (error) {
          if (error instanceof ProjectLifecycleProtocolError && error.zeroEffects) throw error;
          const recovered = await recoverProjectLifecycleDirectFailure(pending.operationId);
          if (!recovered) throw error;
          settlement = recovered;
        }
        if (settlement.status === "stale" || !pending.isCurrent()) {
          return { status: "stale" };
        }
        const claimed = claimProjectLifecycleInitiatorSettlement(pending.operationId);
        if (!claimed) return { status: "stale" };
        if (operation.isCurrent()) setProjects(managedProjectsFromSettlement(claimed));
        if (claimed.result.outcome === "committed") return { status: "committed" };
        return {
          status: "recovery",
          recovery: projectPickerRecoveryPresentation(claimed.result),
        };
      } catch (error) {
        if (
          (error instanceof ProjectLifecycleProtocolError && error.zeroEffects) ||
          (pending && !pending.isCurrent()) ||
          isProjectPickerStaleError(error)
        ) {
          return { status: "stale" };
        }
        return {
          status: "failed",
          error: projectPickerErrorPresentation(error),
        };
      } finally {
        operation?.finish();
      }
    },
    [beginOperation, currentProjectId],
  );

  const toggleFavorite = useCallback(
    (id: string) =>
      runPageOperation({ operation: "favorite", projectId: id }, async (operation) => {
        const isFavorite = await ProjectService.toggleRegisteredProjectFavorite(id);
        if (!operation.isCurrent()) return { status: "stale" };
        setProjects((previous) =>
          previous.map((project) => (project.id === id ? { ...project, isFavorite } : project)),
        );
        return { status: "completed" };
      }),
    [runPageOperation],
  );

  const revealProjectInExplorer = useCallback(
    (projectPath: string) =>
      runPageOperation({ operation: "reveal", projectPath }, async (operation) => {
        const result = await revealPath(projectPath);
        if (!operation.isCurrent()) return { status: "stale" };
        if (!result.ok) throw new Error(result.failure.code);
        return { status: "completed" };
      }),
    [runPageOperation],
  );

  return {
    busy,
    projects,
    pageIssue,
    dismissPageIssue,
    createProject,
    importProjectFromDisk,
    openRecentProject,
    refresh,
    scanProjectsFromFolder,
    cleanupInvalidProjects,
    removeProject,
    deleteProjectFiles,
    toggleFavorite,
    revealProjectInExplorer,
  };
}
