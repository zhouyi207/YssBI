import { useCallback, useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router";
import { toast } from "sonner";
import { useProjectIOStore } from "@/features/core/dataStore";
import { ProjectService } from "@/services/project/projectService";

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
  const file = normalized.split("/").filter(Boolean).pop() ?? path;
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

function writeRecentProjects(projects: ManagedProject[]): void {
  if (typeof localStorage === "undefined") return;
  localStorage.setItem(RECENT_PROJECTS_STORAGE_KEY, JSON.stringify(projects.slice(0, 40)));
}

function upsertRecentProject(path: string): ManagedProject[] {
  const trimmedPath = path.trim();
  const now = new Date().toISOString();
  const previous = readRecentProjects();
  const existing = previous.find((project) => project.path === trimmedPath);
  const nextProject: ManagedProject = {
    id: trimmedPath,
    name: existing?.name ?? pathFileName(trimmedPath),
    path: trimmedPath,
    lastOpenedAt: now,
    isFavorite: existing?.isFavorite ?? false,
  };
  const next = [nextProject, ...previous.filter((project) => project.path !== trimmedPath)];
  writeRecentProjects(next);
  return next;
}

export function useProjectPicker() {
  const navigate = useNavigate();
  const syncFromBackend = useProjectIOStore((state) => state.syncFromBackend);
  const currentPath = useProjectIOStore((state) => state.currentPath);
  const [projects, setProjects] = useState<ManagedProject[]>([]);
  const [busy, setBusy] = useState<BusyState>("idle");

  const refresh = useCallback(() => {
    setProjects(readRecentProjects());
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  useEffect(() => {
    if (!currentPath) return;
    setProjects(upsertRecentProject(currentPath));
  }, [currentPath]);

  const currentProjectId = useMemo(
    () => projects.find((project) => project.path === currentPath)?.id ?? null,
    [currentPath, projects],
  );

  const createProject = useCallback(async () => {
    setBusy("new");
    try {
      await ProjectService.newProject();
      await syncFromBackend();
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
      await syncFromBackend();
      setProjects(upsertRecentProject(result.path));
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
      await syncFromBackend();
      setProjects(upsertRecentProject(result.path));
      navigate("/editor");
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy("idle");
    }
  }, [navigate, syncFromBackend]);

  const removeProject = useCallback((id: string) => {
    const next = readRecentProjects().filter((project) => project.id !== id);
    writeRecentProjects(next);
    setProjects(next);
  }, []);

  const toggleFavorite = useCallback((id: string) => {
    const next = readRecentProjects().map((project) =>
      project.id === id ? { ...project, isFavorite: !project.isFavorite } : project,
    );
    writeRecentProjects(next);
    setProjects(next);
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
