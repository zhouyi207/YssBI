import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";
import { toast } from "sonner";
import { useProjectIOStore } from "@/features/core/dataStore";
import {
  applyCleanupProgressEvent,
  applyScanProgressEvent,
  markProjectPickerProgressDone,
  runWithProjectPickerProgress,
  updateOpenProjectProgressStage,
} from "@/features/application/project/projectPickerProgress";
import {
  ProjectService,
  isPickerTaskCancelledError,
  type ProjectRecordRow,
} from "@/services/project/projectService";
import { formatErrorMessage } from "@/shared/utils/formatErrorMessage";
import { formatDisplayPath, pathsEqualForCompare } from "@/shared/utils/formatDisplayPath";

const RECENT_PROJECTS_STORAGE_KEY = "yssbi-recent-projects";

export interface ManagedProject {
  id: string;
  name: string;
  path: string;
  lastOpenedAt: string;
  isFavorite?: boolean;
}

type BusyState = "idle" | "new" | "open" | "scan" | "import" | "cleanup";

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

function readRecentProjects(): ManagedProject[] {
  if (typeof localStorage === "undefined") return [];
  try {
    const raw = localStorage.getItem(RECENT_PROJECTS_STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as ManagedProject[];
    return Array.isArray(parsed)
      ? parsed
          .filter((item) => item && typeof item.path === "string" && item.path.trim())
          .map((item) => ({ ...item, path: formatDisplayPath(item.path) }))
      : [];
  } catch {
    return [];
  }
}

function clearLegacyRecentProjects(): void {
  if (typeof localStorage === "undefined") return;
  localStorage.removeItem(RECENT_PROJECTS_STORAGE_KEY);
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

export function useProjectPicker() {
  const navigate = useNavigate();
  const { t } = useTranslation();
  const loadProject = useProjectIOStore((state) => state.loadProject);
  const currentPath = useProjectIOStore((state) => state.currentPath);
  const [projects, setProjects] = useState<ManagedProject[]>([]);
  const [busy, setBusy] = useState<BusyState>("idle");

  const refresh = useCallback(async () => {
    try {
      setProjects(await listManagedProjects());
    } catch (error) {
      toast.error(formatErrorMessage(error));
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const legacyProjects = readRecentProjects();
      if (legacyProjects.length > 0) {
        try {
          await ProjectService.migrateLegacyRegisteredProjects(legacyProjects);
          clearLegacyRecentProjects();
        } catch (error) {
          toast.error(formatErrorMessage(error));
        }
      }
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

  const handlePickerTaskCancelled = useCallback(async (messageKey: "scanCancelled" | "cleanupCancelled") => {
    setProjects(await listManagedProjects());
    toast.info(t(`projectPicker.${messageKey}`));
  }, [t]);

  const scanProjectsFromFolder = useCallback(async () => {
    const directory = await ProjectService.pickProjectScanDirectory(
      t("projectPicker.scanFolderTitle"),
    );
    if (!directory) return;

    setBusy("scan");
    try {
      const { result, cancelled } = await runWithProjectPickerProgress(
        {
          initial: {
            stage: t("projectPicker.loading.scanning"),
            detail: t("projectPicker.loading.scanningFolder"),
            cancelable: true,
          },
          onCancel: () => {
            void ProjectService.cancelProjectPickerTask();
          },
        },
        async ({ update, isCancelled }) => {
          const scanResult = await ProjectService.scanProjectsInDirectory(directory, (event) => {
            if (isCancelled()) return;
            applyScanProgressEvent(t, event, update);
          });
          if (!isCancelled()) {
            markProjectPickerProgressDone(t, update);
          }
          return scanResult;
        },
      );

      if (cancelled) {
        await handlePickerTaskCancelled("scanCancelled");
        return;
      }

      setProjects(await listManagedProjects());

      if (result.discovered === 0) {
        toast.info(t("projectPicker.scanNoneFound"));
      } else if (result.newlyRegistered > 0) {
        toast.success(
          t("projectPicker.scanSuccess", {
            added: result.newlyRegistered,
            found: result.discovered,
          }),
        );
      } else {
        toast.info(
          t("projectPicker.scanAlreadyRegistered", { found: result.discovered }),
        );
      }
    } catch (error) {
      if (isPickerTaskCancelledError(error)) {
        await handlePickerTaskCancelled("scanCancelled");
        return;
      }
      toast.error(formatErrorMessage(error));
    } finally {
      setBusy("idle");
    }
  }, [handlePickerTaskCancelled, t]);

  const cleanupInvalidProjects = useCallback(async () => {
    setBusy("cleanup");
    try {
      const { result, cancelled } = await runWithProjectPickerProgress(
        {
          initial: {
            stage: t("projectPicker.loading.cleanup"),
            cancelable: true,
          },
          onCancel: () => {
            void ProjectService.cancelProjectPickerTask();
          },
        },
        async ({ update, isCancelled }) => {
          const cleanupResult = await ProjectService.cleanupInvalidRegisteredProjects((event) => {
            if (isCancelled()) return;
            applyCleanupProgressEvent(t, event, update);
          });
          if (!isCancelled()) {
            markProjectPickerProgressDone(t, update);
          }
          return cleanupResult;
        },
      );

      if (cancelled) {
        await handlePickerTaskCancelled("cleanupCancelled");
        return;
      }

      setProjects(await listManagedProjects());

      if (result.removed === 0) {
        toast.info(t("projectPicker.cleanupNone"));
      } else {
        toast.success(t("projectPicker.cleanupSuccess", { count: result.removed }));
      }
    } catch (error) {
      if (isPickerTaskCancelledError(error)) {
        await handlePickerTaskCancelled("cleanupCancelled");
        return;
      }
      toast.error(formatErrorMessage(error));
    } finally {
      setBusy("idle");
    }
  }, [handlePickerTaskCancelled, t]);

  const createProject = useCallback(async (name: string, path: string) => {
    setBusy("new");
    try {
      await runWithProjectPickerProgress(
        {
          initial: {
            stage: t("projectPicker.loading.creating"),
            percent: 0.2,
          },
        },
        async ({ update }) => {
          const row = await ProjectService.createProject(name, path);
          markProjectPickerProgressDone(t, update);
          setProjects((previous) => [
            rowToManagedProject(row),
            ...previous.filter((project) => project.id !== row.id),
          ]);
          toast.success(t("projectPicker.createSuccess", { name: row.name }));
        },
      );
    } finally {
      setBusy("idle");
    }
  }, [t]);

  const openProjectAtPath = useCallback(async (path: string) => {
    setBusy("open");
    try {
      await runWithProjectPickerProgress(
        {
          initial: {
            stage: t("projectPicker.loading.opening"),
            detail: t("projectPicker.loading.readingFile"),
            percent: 0.1,
          },
        },
        async ({ update }) => {
          const result = await ProjectService.loadProjectToState(path);
          updateOpenProjectProgressStage(t, update, "loadingData");
          const row = await ProjectService.registerProject(pathFileName(result.path), result.path);
          const projectData = await loadProject();
          if (!projectData) {
            const loadError = useProjectIOStore.getState().error;
            throw new Error(formatErrorMessage(loadError, "加载项目数据失败"));
          }
          updateOpenProjectProgressStage(t, update, "preparingEditor");
          setProjects((previous) => [
            rowToManagedProject(row),
            ...previous.filter((project) => project.id !== row.id),
          ]);
          markProjectPickerProgressDone(t, update);
          navigate("/editor");
        },
      );
    } catch (error) {
      toast.error(formatErrorMessage(error));
    } finally {
      setBusy("idle");
    }
  }, [navigate, loadProject, t]);

  const importProjectFromDisk = useCallback(async () => {
    const path = await ProjectService.pickProjectMetadataFile();
    if (!path) return;

    setBusy("import");
    try {
      const row = await ProjectService.registerProject(pathFileName(path), path);
      setProjects((previous) => [
        rowToManagedProject(row),
        ...previous.filter((project) => project.id !== row.id),
      ]);
      toast.success(t("projectPicker.importSuccess", { name: row.name }));
    } catch (error) {
      toast.error(formatErrorMessage(error));
    } finally {
      setBusy("idle");
    }
  }, [t]);

  const openRecentProject = useCallback(
    (path: string) => openProjectAtPath(path),
    [openProjectAtPath],
  );

  const removeProject = useCallback((id: string) => {
    void (async () => {
      try {
        await ProjectService.removeRegisteredProject(id);
        setProjects((previous) => previous.filter((project) => project.id !== id));
      } catch (error) {
        toast.error(formatErrorMessage(error));
      }
    })();
  }, []);

  const deleteProjectFiles = useCallback((id: string) => {
    return (async () => {
      try {
        await ProjectService.deleteRegisteredProjectFiles(id);
        setProjects((previous) => previous.filter((project) => project.id !== id));
        toast.success(t("projectPicker.deleteProjectConfirm.success"));
      } catch (error) {
        toast.error(
          `${t("projectPicker.deleteProjectConfirm.failed")}: ${formatErrorMessage(error)}`,
        );
        throw error;
      }
    })();
  }, [t]);

  const toggleFavorite = useCallback((id: string) => {
    void (async () => {
      try {
        const isFavorite = await ProjectService.toggleRegisteredProjectFavorite(id);
        setProjects((previous) =>
          previous.map((project) =>
            project.id === id ? { ...project, isFavorite } : project,
          ),
        );
      } catch (error) {
        toast.error(formatErrorMessage(error));
      }
    })();
  }, []);

  return {
    busy,
    currentProjectId,
    projects,
    createProject,
    importProjectFromDisk,
    openRecentProject,
    refresh,
    scanProjectsFromFolder,
    cleanupInvalidProjects,
    removeProject,
    deleteProjectFiles,
    toggleFavorite,
  };
}
