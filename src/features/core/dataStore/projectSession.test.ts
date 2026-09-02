import { beforeEach, describe, expect, it, vi } from "vitest";
import { ProjectService } from "@/services/project/projectService";
import { startProjectLifecycle } from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import {
  hydrateProjectPath,
  resolveActiveProjectPath,
} from "@/features/application/project/projectSession";

vi.mock("@/services/project/projectService", () => ({
  ProjectService: {
    getProjectPath: vi.fn(),
  },
}));

describe("projectSession", () => {
  beforeEach(() => {
    startProjectLifecycle("project-instance-1");
    useProjectIOStore.setState({ currentPath: null });
    vi.mocked(ProjectService.getProjectPath).mockReset();
  });

  it("returns cached path without calling backend", async () => {
    useProjectIOStore.setState({ currentPath: "D:/demo/metadata.yssbi" });

    const path = await hydrateProjectPath();

    expect(path).toBe("D:/demo/metadata.yssbi");
    expect(ProjectService.getProjectPath).not.toHaveBeenCalled();
  });

  it("hydrates currentPath from backend when projection is missing", async () => {
    vi.mocked(ProjectService.getProjectPath).mockResolvedValue("D:/demo/metadata.yssbi");

    const path = await resolveActiveProjectPath();

    expect(path).toBe("D:/demo/metadata.yssbi");
    expect(useProjectIOStore.getState().currentPath).toBe("D:/demo/metadata.yssbi");
  });

  it("returns null when backend has no active project", async () => {
    vi.mocked(ProjectService.getProjectPath).mockResolvedValue(null);

    const path = await resolveActiveProjectPath();

    expect(path).toBeNull();
    expect(useProjectIOStore.getState().currentPath).toBeNull();
  });

  it("isolates hydration across project identity replacement", async () => {
    let resolveProjectA!: (path: string | null) => void;
    let resolveProjectB!: (path: string | null) => void;
    const projectAPath = new Promise<string | null>((resolve) => {
      resolveProjectA = resolve;
    });
    const projectBPath = new Promise<string | null>((resolve) => {
      resolveProjectB = resolve;
    });
    vi.mocked(ProjectService.getProjectPath).mockImplementation((projectInstanceId) =>
      projectInstanceId === "project-instance-1" ? projectAPath : projectBPath,
    );

    const projectAHydration = hydrateProjectPath();
    startProjectLifecycle("project-instance-2");
    useProjectIOStore.setState({ currentPath: null });
    const projectBHydration = hydrateProjectPath();

    expect(ProjectService.getProjectPath).toHaveBeenNthCalledWith(1, "project-instance-1");
    expect(ProjectService.getProjectPath).toHaveBeenNthCalledWith(2, "project-instance-2");

    resolveProjectA("D:/project-a/metadata.yssbi");
    await expect(projectAHydration).resolves.toBeNull();
    expect(useProjectIOStore.getState().currentPath).toBeNull();

    resolveProjectB("D:/project-b/metadata.yssbi");
    await expect(projectBHydration).resolves.toBe("D:/project-b/metadata.yssbi");
    expect(useProjectIOStore.getState().currentPath).toBe("D:/project-b/metadata.yssbi");
  });
});
