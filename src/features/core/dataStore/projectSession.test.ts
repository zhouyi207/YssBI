import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ProjectService } from '@/services/project/projectService';
import { useProjectIOStore } from './projectIOStore';
import { reconcileProjectPath, resolveActiveProjectPath } from './projectSession';

vi.mock('@/services/project/projectService', () => ({
  ProjectService: {
    getProjectPath: vi.fn(),
  },
}));

describe('projectSession', () => {
  beforeEach(() => {
    useProjectIOStore.setState({ currentPath: null });
    vi.mocked(ProjectService.getProjectPath).mockReset();
  });

  it('returns cached path without calling backend', async () => {
    useProjectIOStore.setState({ currentPath: 'D:/demo/metadata.yssbi' });

    const path = await reconcileProjectPath();

    expect(path).toBe('D:/demo/metadata.yssbi');
    expect(ProjectService.getProjectPath).not.toHaveBeenCalled();
  });

  it('hydrates currentPath from backend when projection is missing', async () => {
    vi.mocked(ProjectService.getProjectPath).mockResolvedValue('D:/demo/metadata.yssbi');

    const path = await resolveActiveProjectPath();

    expect(path).toBe('D:/demo/metadata.yssbi');
    expect(useProjectIOStore.getState().currentPath).toBe('D:/demo/metadata.yssbi');
  });

  it('returns null when backend has no active project', async () => {
    vi.mocked(ProjectService.getProjectPath).mockResolvedValue(null);

    const path = await resolveActiveProjectPath();

    expect(path).toBeNull();
    expect(useProjectIOStore.getState().currentPath).toBeNull();
  });
});
