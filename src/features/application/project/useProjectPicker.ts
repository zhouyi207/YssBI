import { useCallback, useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router";
import { toast } from "sonner";
import { useProjectIOStore } from "@/features/core/dataStore";
import { ProjectService, type ProjectRecordRow } from "@/services/project/projectService";

const RECENT_PROJECTS_STORAGE_KEY = "yssbi-recent-projects";

export interface ManagedProject {
  id: string;
  name: string;
  path: string;
  lastOpenedAt: string;
  isFavorite?: boolean;
}

type BusyState = "idle" | "new" | "open";

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
      ? parsed.filter((item) => item && typeof item.path === "string" && item.path.trim())
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
    path: row.path,
    lastOpenedAt: row.lastOpenedAt ?? row.createdAt,
    isFavorite: row.isFavorite,
  };
}

export function useProjectPicker() {
  const navigate = useNavigate();
  const syncFromBackend = useProjectIOStore((state) => state.syncFromBackend);
  const currentPath = useProjectIOStore((state) => state.currentPath);
  const [projects, setProjects] = useState<ManagedProject[]>([]);
  const [busy, setBusy] = useState<BusyState>("idle");

  const refresh = useCallback(async () => {
    try {
      const rows = await ProjectService.listRegisteredProjects();
      setProjects(rows.map(rowToManagedProject));
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
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
          toast.error(error instanceof Error ? error.message : String(error));
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
    () => projects.find((project) => project.path === currentPath)?.id ?? null,
    [currentPath, projects],
  );

  const createProject = useCallback(async (name: string, path: string) => {
    setBusy("new");
    try {
      const row = await ProjectService.createProject(name, path);
      await syncFromBackend();
      setProjects((previous) => [
        rowToManagedProject(row),
        ...previous.filter((project) => project.id !== row.id),
      ]);
      navigate("/editor");
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy("idle");
    }
  }, [navigate, syncFromBackend]);

  const openProjectFromDisk = useCallback(async () => {
    setBusy("open");
    try {
      const result = await ProjectService.loadProjectToState();
      if (!result) return;
      const row = await ProjectService.registerProject(pathFileName(result.path), result.path);
      await syncFromBackend();
      setProjects((previous) => [
        rowToManagedProject(row),
        ...previous.filter((project) => project.id !== row.id),
      ]);
      navigate("/editor");
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy("idle");
    }
  }, [navigate, syncFromBackend]);

  const openRecentProject = useCallback(async (path: string) => {
    setBusy("open");
    try {
      const result = await ProjectService.loadProjectToState(path);
      if (!result) return;
      const row = await ProjectService.registerProject(pathFileName(result.path), result.path);
      await syncFromBackend();
      setProjects((previous) => [
        rowToManagedProject(row),
        ...previous.filter((project) => project.id !== row.id),
      ]);
      navigate("/editor");
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy("idle");
    }
  }, [navigate, syncFromBackend]);

  const removeProject = useCallback((id: string) => {
    void (async () => {
      try {
        await ProjectService.removeRegisteredProject(id);
        setProjects((previous) => previous.filter((project) => project.id !== id));
      } catch (error) {
        toast.error(error instanceof Error ? error.message : String(error));
      }
    })();
  }, []);

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
        toast.error(error instanceof Error ? error.message : String(error));
      }
    })();
  }, []);

  return {
    busy,
    currentProjectId,
    projects,
    createProject,
    openProjectFromDisk,
    openRecentProject,
    refresh,
    removeProject,
    toggleFavorite,
  };
}
