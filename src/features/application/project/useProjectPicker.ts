import { useCallback, useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router";
import { loadActivatedProject, useProjectIOStore } from '@/features/application/project/projectIOStore';
import {
  applyCleanupProgressEvent,
  applyScanProgressEvent,
  markProjectPickerProgressDone,
  projectPickerProgressInitial,
  projectPickerScanFolderTitle,
  runWithProjectPickerProgress,
  updateOpenProjectProgressStage,
} from "@/features/application/project/projectPickerProgress";
import {
  ProjectService,
  isPickerTaskCancelledError,
} from "@/services/project/projectService";
import { revealPath } from '@/services/platform/opener';
import { openPathDialog } from '@/services/platform/pathDialog';
import type { ProjectRecordRow } from "@/shared/types/domain/project";
import { formatDisplayPath, pathsEqualForCompare } from "@/shared/utils/formatDisplayPath";
import {
  ProjectPickerOperationError,
  isProjectPickerStaleError,
  projectPickerErrorPresentation,
  projectPickerRecoveryPresentation,
  type ProjectPickerLifecycleActionOutcome,
  type ProjectPickerPageActionOutcome,
  type ProjectPickerPageIssue,
} from "./projectPickerOutcomes";

import {
  ProjectLifecycleProtocolError,
  applyProjectLifecycleReceipt,
  claimProjectLifecycleInitiatorSettlement,
  recoverProjectLifecycleDirectFailure,
  registerPendingProjectLifecycleOperation,
  type PendingProjectLifecycleOperation,
  type ProjectLifecycleReceiptSettlement,
} from '@/features/application/projectLifecycleReceipt';
import { createProjectLifecycleReceiptDependencies } from '@/features/application/projectLifecycleReceiptDependencies';

export interface ManagedProject {
  id: string;
  name: string;
  path: string;
  lastOpenedAt: string;
  isFavorite?: boolean;
}

type BusyState = "idle" | "new" | "open" | "scan" | "import" | "cleanup";

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
    path: formatDisplayPath(row.path),
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
    throw new Error('Lifecycle settlement omitted registry projection');
  }
  return settlement.registryProjects.map(rowToManagedProject);
}

export function useProjectPicker() {
  const navigate = useNavigate();
  const currentPath = useProjectIOStore((state) => state.currentPath);
  const [projects, setProjects] = useState<ManagedProject[]>([]);
  const [busy, setBusy] = useState<BusyState>("idle");
  const [pageIssue, setPageIssue] = useState<ProjectPickerPageIssue | null>(null);

  const publishPageIssue = useCallback((
    issue: ProjectPickerPageIssue,
  ): ProjectPickerPageActionOutcome => {
    setPageIssue(issue);
    return { status: 'issue', issue };
  }, []);

  const dismissPageIssue = useCallback(() => {
    setPageIssue(null);
  }, []);

  const refresh = useCallback(async (): Promise<ProjectPickerPageActionOutcome> => {
    setPageIssue(null);
    try {
      setProjects(await listManagedProjects());
      return { status: 'completed' };
    } catch (error) {
      return publishPageIssue({
        kind: 'failure',
        operation: 'refresh',
        error: projectPickerErrorPresentation(error),
      });
    }
  }, [publishPageIssue]);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      if (!cancelled) {
        await refresh();
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [refresh]);

  useEffect(() => {
    if (!currentPath) return;
    void (async () => {
      try {
        const row = await ProjectService.registerProject(pathFileName(currentPath), currentPath);
        setProjects((previous) => [
          rowToManagedProject(row),
          ...previous.filter((project) => project.id !== row.id),
        ]);
      } catch {
        await refresh();
      }
    })();
  }, [currentPath, refresh]);

  const currentProjectId = useMemo(
    () => projects.find((project) => pathsEqualForCompare(project.path, currentPath ?? ""))?.id ?? null,
    [currentPath, projects],
  );

  const handlePickerTaskCancelled = useCallback(async (): Promise<ProjectPickerPageActionOutcome> => {
    try {
      setProjects(await listManagedProjects());
      return { status: 'cancelled' };
    } catch (error) {
      return publishPageIssue({
        kind: 'failure',
        operation: 'refresh',
        error: projectPickerErrorPresentation(error),
      });
    }
  }, [publishPageIssue]);

  const scanProjectsFromFolder = useCallback(async (): Promise<ProjectPickerPageActionOutcome> => {
    setPageIssue(null);
    try {
      const selection = await openPathDialog({
        directory: true,
        multiple: false,
        title: projectPickerScanFolderTitle(),
      });
      if (!selection.ok) throw new Error(selection.failure.code);
      const directory = selection.value;
      if (!directory) return { status: 'cancelled' };
      if (Array.isArray(directory)) return { status: 'cancelled' };

      setBusy("scan");
      const { result, cancelled } = await runWithProjectPickerProgress(
        {
          initial: projectPickerProgressInitial('scan'),
          onCancel: () => {
            void ProjectService.cancelProjectPickerTask();
          },
        },
        async ({ update, isCancelled }) => {
          try {
            const scanResult = await ProjectService.scanProjectsInDirectory(directory, (event) => {
              if (isCancelled()) return;
              applyScanProgressEvent(event, update);
            });
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

      if (cancelled) return await handlePickerTaskCancelled();

      setProjects(await listManagedProjects());
      if (result.discovered === 0) {
        return publishPageIssue({
          kind: 'empty',
          operation: 'scan',
          reason: 'noneFound',
          found: 0,
        });
      }
      if (result.newlyRegistered === 0) {
        return publishPageIssue({
          kind: 'empty',
          operation: 'scan',
          reason: 'alreadyRegistered',
          found: result.discovered,
        });
      }
      return { status: 'completed' };
    } catch (error) {
      if (error instanceof ProjectPickerTaskCancelled || isPickerTaskCancelledError(error)) {
        return await handlePickerTaskCancelled();
      }
      if (isProjectPickerStaleError(error)) return { status: 'stale' };
      return publishPageIssue({
        kind: 'failure',
        operation: 'scan',
        error: projectPickerErrorPresentation(error),
      });
    } finally {
      setBusy("idle");
    }
  }, [handlePickerTaskCancelled, publishPageIssue]);

  const cleanupInvalidProjects = useCallback(async (): Promise<ProjectPickerPageActionOutcome> => {
    setPageIssue(null);
    setBusy("cleanup");
    try {
      const { result, cancelled } = await runWithProjectPickerProgress(
        {
          initial: projectPickerProgressInitial('cleanup'),
          onCancel: () => {
            void ProjectService.cancelProjectPickerTask();
          },
        },
        async ({ update, isCancelled }) => {
          try {
            const cleanupResult = await ProjectService.cleanupInvalidRegisteredProjects((event) => {
              if (isCancelled()) return;
              applyCleanupProgressEvent(event, update);
            });
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

      if (cancelled) return await handlePickerTaskCancelled();

      setProjects(await listManagedProjects());
      if (result.removed === 0) {
        return publishPageIssue({
          kind: 'empty',
          operation: 'cleanup',
          reason: 'noneFound',
        });
      }
      return { status: 'completed' };
    } catch (error) {
      if (error instanceof ProjectPickerTaskCancelled || isPickerTaskCancelledError(error)) {
        return await handlePickerTaskCancelled();
      }
      if (isProjectPickerStaleError(error)) return { status: 'stale' };
      return publishPageIssue({
        kind: 'failure',
        operation: 'cleanup',
        error: projectPickerErrorPresentation(error),
      });
    } finally {
      setBusy("idle");
    }
  }, [handlePickerTaskCancelled, publishPageIssue]);

  const createProject = useCallback(async (
    name: string,
    path: string,
  ): Promise<ProjectPickerLifecycleActionOutcome> => {
    let pending: PendingProjectLifecycleOperation | undefined;
    try {
      pending = registerPendingProjectLifecycleOperation({
        kind: 'create',
        expectsActiveProject: false,
      });
      setBusy("new");
      const progress = await runWithProjectPickerProgress(
        {
          initial: projectPickerProgressInitial('create'),
        },
        async ({ update }): Promise<ProjectPickerLifecycleActionOutcome> => {
          let settlement: ProjectLifecycleReceiptSettlement;
          try {
            const result = await ProjectService.createProject(name, path, pending!.operationId);
            if (!pending!.isCurrent()) return { status: 'stale' };
            settlement = await applyProjectLifecycleReceipt(
              result,
              'direct',
              createProjectLifecycleReceiptDependencies(),
            );
          } catch (error) {
            if (error instanceof ProjectLifecycleProtocolError && error.zeroEffects) throw error;
            const recovered = await recoverProjectLifecycleDirectFailure(pending!.operationId);
            if (!recovered) throw error;
            settlement = recovered;
          }
          if (settlement.status === 'stale' || !pending!.isCurrent()) {
            return { status: 'stale' };
          }
          const claimed = claimProjectLifecycleInitiatorSettlement(pending!.operationId);
          if (!claimed) return { status: 'stale' };
          setProjects(managedProjectsFromSettlement(claimed));
          markProjectPickerProgressDone(update);
          if (claimed.result.outcome === 'committed' && claimed.result.record) {
            return { status: 'committed' };
          }
          return {
            status: 'recovery',
            recovery: projectPickerRecoveryPresentation(claimed.result),
          };
        },
      );
      return progress.result;
    } catch (error) {
      if ((error instanceof ProjectLifecycleProtocolError && error.zeroEffects)
        || (pending && !pending.isCurrent())
        || isProjectPickerStaleError(error)) {
        return { status: 'stale' };
      }
      return {
        status: 'failed',
        error: projectPickerErrorPresentation(error),
      };
    } finally {
      setBusy("idle");
    }
  }, []);

  const openProjectAtPath = useCallback(async (
    path: string,
  ): Promise<ProjectPickerPageActionOutcome> => {
    setPageIssue(null);
    setBusy("open");
    try {
      const progress = await runWithProjectPickerProgress(
        {
          initial: projectPickerProgressInitial('open'),
        },
        async ({ update }): Promise<ProjectPickerPageActionOutcome> => {
          const result = await ProjectService.loadProjectToState(path);
          updateOpenProjectProgressStage(update, "loadingData");
          const row = await ProjectService.registerProject(pathFileName(result.path), result.path);
          const projectData = await loadActivatedProject(result);
          if (!projectData) {
            if (!useProjectIOStore.getState().error) return { status: 'stale' };
            throw new ProjectPickerOperationError('project_activation_failed');
          }
          updateOpenProjectProgressStage(update, "preparingEditor");
          setProjects((previous) => [
            rowToManagedProject(row),
            ...previous.filter((project) => project.id !== row.id),
          ]);
          markProjectPickerProgressDone(update);
          navigate("/editor");
          return { status: 'completed' };
        },
      );
      return progress.result;
    } catch (error) {
      if (isProjectPickerStaleError(error)) return { status: 'stale' };
      return publishPageIssue({
        kind: 'failure',
        operation: 'open',
        projectPath: path,
        error: projectPickerErrorPresentation(error),
      });
    } finally {
      setBusy("idle");
    }
  }, [navigate, publishPageIssue]);

  const importProjectFromDisk = useCallback(async (): Promise<ProjectPickerPageActionOutcome> => {
    setPageIssue(null);
    try {
      const selection = await openPathDialog({
        multiple: false,
        filters: [{ name: 'YssBI Project', extensions: ['yssbi'] }],
      });
      if (!selection.ok) throw new Error(selection.failure.code);
      const path = selection.value;
      if (!path) return { status: 'cancelled' };
      if (Array.isArray(path)) return { status: 'cancelled' };

      setBusy("import");
      const row = await ProjectService.registerProject(pathFileName(path), path);
      setProjects((previous) => [
        rowToManagedProject(row),
        ...previous.filter((project) => project.id !== row.id),
      ]);
      return { status: 'completed' };
    } catch (error) {
      if (isProjectPickerStaleError(error)) return { status: 'stale' };
      return publishPageIssue({
        kind: 'failure',
        operation: 'import',
        error: projectPickerErrorPresentation(error),
      });
    } finally {
      setBusy("idle");
    }
  }, [publishPageIssue]);

  const openRecentProject = useCallback(
    (path: string) => openProjectAtPath(path),
    [openProjectAtPath],
  );

  const removeProject = useCallback(async (
    id: string,
  ): Promise<ProjectPickerPageActionOutcome> => {
    setPageIssue(null);
    try {
      await ProjectService.removeRegisteredProject(id);
      setProjects((previous) => previous.filter((project) => project.id !== id));
      return { status: 'completed' };
    } catch (error) {
      if (isProjectPickerStaleError(error)) return { status: 'stale' };
      return publishPageIssue({
        kind: 'failure',
        operation: 'remove',
        projectId: id,
        error: projectPickerErrorPresentation(error),
      });
    }
  }, [publishPageIssue]);

  const deleteProjectFiles = useCallback(async (
    id: string,
  ): Promise<ProjectPickerLifecycleActionOutcome> => {
    let pending: PendingProjectLifecycleOperation | undefined;
    try {
      const active = id === currentProjectId;
      pending = registerPendingProjectLifecycleOperation({
        kind: 'delete',
        expectsActiveProject: active,
      });
      let settlement: ProjectLifecycleReceiptSettlement;
      try {
        const result = await ProjectService.deleteRegisteredProjectFiles(
          id,
          active ? pending.projectInstanceId : null,
          pending.operationId,
        );
        if (!pending.isCurrent()) return { status: 'stale' };
        settlement = await applyProjectLifecycleReceipt(
          result,
          'direct',
          createProjectLifecycleReceiptDependencies(),
        );
      } catch (error) {
        if (error instanceof ProjectLifecycleProtocolError && error.zeroEffects) throw error;
        const recovered = await recoverProjectLifecycleDirectFailure(pending.operationId);
        if (!recovered) throw error;
        settlement = recovered;
      }
      if (settlement.status === 'stale' || !pending.isCurrent()) {
        return { status: 'stale' };
      }
      const claimed = claimProjectLifecycleInitiatorSettlement(pending.operationId);
      if (!claimed) return { status: 'stale' };
      setProjects(managedProjectsFromSettlement(claimed));
      if (claimed.result.outcome === 'committed') return { status: 'committed' };
      return {
        status: 'recovery',
        recovery: projectPickerRecoveryPresentation(claimed.result),
      };
    } catch (error) {
      if ((error instanceof ProjectLifecycleProtocolError && error.zeroEffects)
        || (pending && !pending.isCurrent())
        || isProjectPickerStaleError(error)) {
        return { status: 'stale' };
      }
      return {
        status: 'failed',
        error: projectPickerErrorPresentation(error),
      };
    }
  }, [currentProjectId]);

  const toggleFavorite = useCallback(async (
    id: string,
  ): Promise<ProjectPickerPageActionOutcome> => {
    setPageIssue(null);
    try {
      const isFavorite = await ProjectService.toggleRegisteredProjectFavorite(id);
      setProjects((previous) =>
        previous.map((project) =>
          project.id === id ? { ...project, isFavorite } : project,
        ),
      );
      return { status: 'completed' };
    } catch (error) {
      if (isProjectPickerStaleError(error)) return { status: 'stale' };
      return publishPageIssue({
        kind: 'failure',
        operation: 'favorite',
        projectId: id,
        error: projectPickerErrorPresentation(error),
      });
    }
  }, [publishPageIssue]);

  const revealProjectInExplorer = useCallback(async (
    projectPath: string,
  ): Promise<ProjectPickerPageActionOutcome> => {
    setPageIssue(null);
    try {
      const result = await revealPath(projectPath);
      if (!result.ok) throw new Error(result.failure.code);
      return { status: 'completed' };
    } catch (error) {
      return publishPageIssue({
        kind: 'failure',
        operation: 'reveal',
        projectPath,
        error: projectPickerErrorPresentation(error),
      });
    }
  }, [publishPageIssue]);

  return {
    busy,
    currentProjectId,
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
