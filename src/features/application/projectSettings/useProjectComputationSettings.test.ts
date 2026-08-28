// @vitest-environment happy-dom
import { act, createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  ComputationSettingsMutationReceiptDto,
  ComputationSettingsSnapshotDto,
} from '@/shared/types/dto/projectComputationSettings';
import { ProjectService } from '@/services/project/projectService';
import { uiStore } from '@/features/core/ui/UIStore';
import {
  reconcileProjectComputationSettingsEvent,
  useProjectComputationSettings,
} from './useProjectComputationSettings';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const project = vi.hoisted(() => ({ id: 'project-a' as string | null, epoch: 1 }));

vi.mock('@/features/application/project/projectIOStore', () => ({
  useProjectIOStore: <T,>(selector: (state: { projectInstanceId: string | null }) => T): T => selector({
    projectInstanceId: project.id,
  }),
}));

vi.mock('@/features/core/projectLifecycle/projectLifecycleAuthority', () => ({
  captureProjectIdentity: vi.fn(() => ({ projectInstanceId: project.id, epoch: project.epoch })),
  isCurrentProjectIdentity: vi.fn((identity: { projectInstanceId: string; epoch: number }) => (
    identity.projectInstanceId === project.id && identity.epoch === project.epoch
  )),
}));

vi.mock('@/services/project/projectService', () => ({
  ProjectService: {
    getProjectComputationSettings: vi.fn(),
    updateProjectComputationSettings: vi.fn(),
  },
}));

function snapshot(overrides: Partial<ComputationSettingsSnapshotDto> = {}): ComputationSettingsSnapshotDto {
  return {
    projectInstanceId: 'project-a',
    settingsRevision: 3,
    publicationRevision: 9,
    settings: {
      numeric: { tolerance: { absolute: 1e-12, relative: 1e-9 } },
      missingValues: { statistics: 'listwise' },
    },
    ...overrides,
  };
}

function receipt(operationId: string, revision = 4): ComputationSettingsMutationReceiptDto {
  return {
    ...snapshot({ settingsRevision: revision, publicationRevision: 10 }),
    operationId,
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => { resolve = resolvePromise; });
  return { promise, resolve };
}

describe('useProjectComputationSettings', () => {
  let host: HTMLDivElement;
  let root: Root;
  let current: ReturnType<typeof useProjectComputationSettings> | undefined;

  function Harness() {
    current = useProjectComputationSettings();
    return null;
  }

  async function render() {
    await act(async () => {
      root.render(createElement(Harness));
      await Promise.resolve();
    });
  }

  beforeEach(() => {
    vi.clearAllMocks();
    project.id = 'project-a';
    project.epoch = 1;
    current = undefined;
    vi.mocked(ProjectService.getProjectComputationSettings).mockResolvedValue(snapshot());
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it('loads authoritative settings for the active project and never reads localStorage', async () => {
    const localStorageRead = vi.spyOn(Storage.prototype, 'getItem');
    await render();

    expect(ProjectService.getProjectComputationSettings).toHaveBeenCalledWith('project-a');
    expect(current?.confirmed?.settingsRevision).toBe(3);
    expect(current?.draft).toMatchObject({ absolute: '1e-12', relative: '1e-9', statistics: 'listwise' });
    expect(localStorageRead).not.toHaveBeenCalled();
  });

  it('does not query and remains disabled without an active project', async () => {
    project.id = null;
    await render();
    expect(ProjectService.getProjectComputationSettings).not.toHaveBeenCalled();
    expect(current?.enabled).toBe(false);
  });

  it('ignores a load response after project replacement', async () => {
    const request = deferred<ComputationSettingsSnapshotDto>();
    vi.mocked(ProjectService.getProjectComputationSettings).mockReturnValue(request.promise);
    await render();
    project.id = 'project-b';
    project.epoch += 1;
    await act(async () => request.resolve(snapshot()));
    expect(current?.confirmed).toBeNull();
  });

  it('validates finite nonnegative tolerances and rejects a zero pair', async () => {
    await render();
    act(() => current?.setDraft({ absolute: '-1' }));
    expect(current?.validationError).toMatch(/finite and nonnegative/i);
    act(() => current?.setDraft({ absolute: '0', relative: '0' }));
    expect(current?.validationError).toMatch(/cannot both be zero/i);
    act(() => current?.setDraft({ absolute: '1e-8', relative: '0' }));
    expect(current?.validationError).toBeNull();
  });

  it('applies one correlated revisioned mutation and reconciles its event echo once', async () => {
    await render();
    act(() => current?.setDraft({ absolute: '1e-8', statistics: 'reject' }));
    vi.mocked(ProjectService.updateProjectComputationSettings).mockImplementation(async (request) => {
      const result = receipt(request.operationId);
      reconcileProjectComputationSettingsEvent(result);
      return result;
    });

    await act(async () => { await current?.apply(); });

    expect(ProjectService.updateProjectComputationSettings).toHaveBeenCalledWith(expect.objectContaining({
      projectInstanceId: 'project-a',
      expectedRevision: 3,
      operationId: expect.any(String),
      settings: {
        numeric: { tolerance: { absolute: 1e-8, relative: 1e-9 } },
        missingValues: { statistics: 'reject' },
      },
    }));
    expect(current?.confirmed?.settingsRevision).toBe(4);
    expect(current?.isDirty).toBe(false);
  });

  it('ignores mismatched operation receipts and older event revisions', async () => {
    await render();
    act(() => current?.setDraft({ absolute: '1e-8' }));
    vi.mocked(ProjectService.updateProjectComputationSettings).mockImplementation(async () => (
      receipt('different-operation')
    ));
    await act(async () => { await expect(current?.apply()).rejects.toThrow(/correlation/i); });
    expect(current?.confirmed?.settingsRevision).toBe(3);

    act(() => reconcileProjectComputationSettingsEvent(receipt('old-event', 2)));
    expect(current?.confirmed?.settingsRevision).toBe(3);
  });

  it('rejects a correlated direct receipt whose settings revision does not advance', async () => {
    await render();
    act(() => current?.setDraft({ absolute: '1e-8' }));
    vi.mocked(ProjectService.updateProjectComputationSettings).mockImplementation(async (request) => (
      receipt(request.operationId, 3)
    ));

    await act(async () => { await expect(current?.apply()).rejects.toThrow(/revision/i); });
    expect(current?.confirmed?.settingsRevision).toBe(3);
    expect(current?.isDirty).toBe(true);
  });

  it('asks before discarding a dirty draft when the active project changes', async () => {
    await render();
    act(() => current?.setDraft({ absolute: '1e-8' }));
    const confirm = vi.spyOn(uiStore, 'confirm').mockResolvedValue(false);
    project.id = 'project-b';
    project.epoch += 1;

    await act(async () => {
      root.render(createElement(Harness));
      await Promise.resolve();
    });

    expect(confirm).toHaveBeenCalledWith(expect.objectContaining({
      title: 'Discard computation changes?',
    }));
    expect(ProjectService.getProjectComputationSettings).not.toHaveBeenCalledWith('project-b');
    expect(current?.draft.absolute).toBe('1e-8');
    expect(current?.enabled).toBe(false);
  });

  it('restores the recommended values into the local draft without persisting', async () => {
    await render();
    act(() => current?.setDraft({ absolute: '1e-4', relative: '1e-5', statistics: 'reject' }));
    act(() => current?.restoreRecommended());
    expect(current?.draft).toEqual({ absolute: '1e-12', relative: '1e-9', statistics: 'listwise' });
    expect(ProjectService.updateProjectComputationSettings).not.toHaveBeenCalled();
  });
});
