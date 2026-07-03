import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";
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
import { uiStore } from "@/features/core/ui/UIStore";

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
      uiStore.showToast(formatErrorMessage(error), "error");
    }
  }, []);

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

  const handlePickerTaskCancelled = useCallback(async (messageKey: "scanCancelled" | "cleanupCancelled") => {
    setProjects(await listManagedProjects());
    uiStore.showToast(t(`projectPicker.${messageKey}`), "info");
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
        uiStore.showToast(t("projectPicker.scanNoneFound"), "info");
      } else if (result.newlyRegistered > 0) {
        uiStore.showToast(
          t("projectPicker.scanSuccess", {
            added: result.newlyRegistered,
            found: result.discovered,
          }),
          "success",
        );
      } else {
        uiStore.showToast(
          t("projectPicker.scanAlreadyRegistered", { found: result.discovered }),
          "info",
        );
      }
    } catch (error) {
      if (isPickerTaskCancelledError(error)) {
        await handlePickerTaskCancelled("scanCancelled");
        return;
      }
      uiStore.showToast(formatErrorMessage(error), "error");
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
        uiStore.showToast(t("projectPicker.cleanupNone"), "info");
      } else {
        uiStore.showToast(t("projectPicker.cleanupSuccess", { count: result.removed }), "success");
      }
    } catch (error) {
      if (isPickerTaskCancelledError(error)) {
        await handlePickerTaskCancelled("cleanupCancelled");
        return;
      }
      uiStore.showToast(formatErrorMessage(error), "error");
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
          uiStore.showToast(t("projectPicker.createSuccess", { name: row.name }), "success");
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
      uiStore.showToast(formatErrorMessage(error), "error");
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
      uiStore.showToast(t("projectPicker.importSuccess", { name: row.name }), "success");
    } catch (error) {
      uiStore.showToast(formatErrorMessage(error), "error");
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
        uiStore.showToast(formatErrorMessage(error), "error");
      }
    })();
  }, []);

  const deleteProjectFiles = useCallback((id: string) => {
    return (async () => {
      try {
        await ProjectService.deleteRegisteredProjectFiles(id);
        setProjects((previous) => previous.filter((project) => project.id !== id));
        uiStore.showToast(t("projectPicker.deleteProjectConfirm.success"), "success");
      } catch (error) {
        uiStore.showToast(
          `${t("projectPicker.deleteProjectConfirm.failed")}: ${formatErrorMessage(error)}`,
          "error",
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
        uiStore.showToast(formatErrorMessage(error), "error");
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
