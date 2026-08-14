// @vitest-environment happy-dom

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import { buildWorksheetLayoutTab } from '@/features/core/layout/layoutTabModel';

import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import {
  resourceKey,
  useDocumentStateStore,
  useResourceStore,
} from '@/features/core/resource';
import { useEditorStore } from '@/features/core/editor/stores/useEditorStore';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';

import { WorksheetService } from '@/services/worksheet/worksheetService';
import type { WorksheetDocument } from '@/shared/types/domain/worksheet';
import type { ResourceMutationResultDto } from '@/shared/types/dto/editorMutation';
import { performWorksheetDelete } from './closeEditorTab';
import { useWorksheetManagement } from './useWorksheetManagement';

import { resolveTabDisplayName } from './resolveTabDisplayName';
import { commitFileFirstResourceIndex } from '@/features/application/resource/resourceActions';
import {
  beginWorksheetRenameLifecycle,
  clearWorksheetLifecycleProjects,
  isWorksheetLifecycleCurrent,
} from '@/features/application/editor/worksheetLifecycleCoordinator';

vi.mock('react-i18next', async (importOriginal) => ({
  ...(await importOriginal<typeof import('react-i18next')>()),
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock('@/features/core/execution', () => ({
  useExecutionStore: { getState: vi.fn() },
  getExecutionEventGraph: vi.fn(),
  resolveExecutionGraphPath: vi.fn(),
  graphHasClearableArtifacts: vi.fn(),
  enqueueLiveExecutionEvent: vi.fn(),
}));
vi.mock('@/features/application/execution/openInspectableResult', () => ({
  openWindowInspectableResult: vi.fn(),
}));
vi.mock('@/features/application/graphDiagnostics/warnCallFunctionIssues', () => ({
  warnCallFunctionIssuesBeforeSave: vi.fn(),
}));
vi.mock('@/features/application/resource/resourceActions', () => ({
  commitFileFirstResourceIndex: vi.fn(),
}));
vi.mock('./useSidebarTab', () => ({ useSidebarTab: () => vi.fn() }));
vi.mock('./openEditorTab', () => ({ openEditorTab: vi.fn() }));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

const projectA = '00000000-0000-0000-0000-000000000601';
const worksheetPath = 'worksheets/Worksheet.yssbi-worksheet';

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((settle) => { resolve = settle; });
  return { promise, resolve };
}

function worksheet(revision = 0): WorksheetDocument {
  return {
    schemaVersion: 3,
    revision,
    databaseId: 'database-1',
    chartType: 'scatter',
    encodings: { x: 'x', y: 'y' },
  };
}

function worksheetLifecycleDelta(
  document: WorksheetDocument,
  removing = false,
  path = worksheetPath,
  name = 'Worksheet',
  operationId = '00000000-0000-0000-0000-000000000501',
) {
  const state = {
    revision: document.revision,
    path,
    kind: 'worksheet' as const,
    name,
  };
  return {
    resource: { kind: 'worksheet' as const, key: path },
    fromRevision: document.revision,
    toRevision: removing ? document.revision + 1 : document.revision,
    causedBy: operationId,
    payload: {
      kind: 'resource_lifecycle' as const,
      patch: removing ? { before: state, after: null } : { before: null, after: state },
    },
  };
}

function mutation(
  document: WorksheetDocument,
  publicationRevision = 1,
  operationId = '00000000-0000-0000-0000-000000000501',
): ResourceMutationResultDto {
  return {
    operationId,
    projectInstanceId: projectA,
    publicationRevision,
    moves: [],
    deltas: [worksheetLifecycleDelta(document, false, worksheetPath, 'Worksheet', operationId)],
    projectionReplacements: [],
    projectionStatus: { status: 'complete', expectedGraphPaths: [] },
    history: { canUndo: true, canRedo: false },
  };
}

function replaceProject(): void {
  useProjectIOStore.setState({ projectInstanceId: 'project-b' });
  projectPublicationCoordinator.startProject('project-b', 0);
  useWorksheetStore.getState().clear();
}

describe('worksheet rename lifecycle coordinator', () => {
  beforeEach(() => clearWorksheetLifecycleProjects());

  it('owns monotonic tokens independently by project instance and opaque worksheet path', () => {
    const first = beginWorksheetRenameLifecycle(projectA, worksheetPath);
    const second = beginWorksheetRenameLifecycle(projectA, worksheetPath);
    const otherPath = beginWorksheetRenameLifecycle(
      projectA,
      'worksheets/Other Name.yssbi-worksheet',
    );
    const otherProject = beginWorksheetRenameLifecycle('project-b', worksheetPath);

    expect(second).toBeGreaterThan(first);
    expect(otherPath).toBeGreaterThan(second);
    expect(otherProject).toBeGreaterThan(otherPath);
    expect(isWorksheetLifecycleCurrent(projectA, worksheetPath, first)).toBe(false);
    expect(isWorksheetLifecycleCurrent(projectA, worksheetPath, second)).toBe(true);
    expect(isWorksheetLifecycleCurrent(
      projectA,
      'worksheets/Other Name.yssbi-worksheet',
      otherPath,
    )).toBe(true);
    expect(isWorksheetLifecycleCurrent('project-b', worksheetPath, otherProject)).toBe(true);
  });

  it('clears project/path entries on replacement without resetting the monotonic counter', () => {
    projectPublicationCoordinator.cancelProject();
    const beforeReplacement = beginWorksheetRenameLifecycle(projectA, worksheetPath);

    projectPublicationCoordinator.startProject('project-b', 0);
    expect(isWorksheetLifecycleCurrent(projectA, worksheetPath, beforeReplacement)).toBe(false);

    const afterReplacement = beginWorksheetRenameLifecycle('project-b', worksheetPath);
    expect(afterReplacement).toBeGreaterThan(beforeReplacement);
    projectPublicationCoordinator.cancelProject();
    expect(isWorksheetLifecycleCurrent('project-b', worksheetPath, afterReplacement)).toBe(false);

    const afterCancellation = beginWorksheetRenameLifecycle(projectA, worksheetPath);
    expect(afterCancellation).toBeGreaterThan(afterReplacement);
  });
});

describe('worksheet command lifecycle guards', () => {
  let host: HTMLDivElement;
  let root: Root;
  let management!: ReturnType<typeof useWorksheetManagement>;
  const openWorksheet = vi.fn(async () => undefined);

  beforeEach(() => {
    vi.restoreAllMocks();
    vi.clearAllMocks();
    vi.mocked(commitFileFirstResourceIndex).mockResolvedValue(true);
    useWorksheetStore.getState().clear();
    useDocumentStateStore.getState().clear();
    useResourceStore.getState().clear();
    useEditorStore.setState({ detailFocus: null });
    useProjectIOStore.setState({ projectInstanceId: projectA, currentPath: 'C:/project-a' });
    projectPublicationCoordinator.startProject(projectA, 0);
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
    function Harness() {
      management = useWorksheetManagement(openWorksheet);
      return null;
    }
    act(() => root.render(<Harness />));
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it('opens create using the Rust lifecycle path and name', async () => {
    const opaquePath = 'worksheets/Path Does Not Reveal Label.yssbi-worksheet';
    vi.spyOn(WorksheetService, 'loadWorksheet').mockResolvedValue(worksheet());
    vi.spyOn(WorksheetService, 'createWorksheet').mockImplementation(
      async (_projectInstanceId, operationId) => ({
        ...mutation(worksheet(), 1, operationId),
        deltas: [worksheetLifecycleDelta(
          worksheet(),
          false,
          opaquePath,
          'Rust supplied label',
          operationId,
        )],
      }),
    );

    await management.addWorksheet();

    expect(openWorksheet).toHaveBeenCalledWith(opaquePath, 'Rust supplied label');
  });


  it('resolves display names only from authoritative worksheet metadata', () => {
    const opaquePath = 'worksheets/Path Does Not Reveal Label.yssbi-worksheet';
    useWorksheetStore.getState().setIndex([{
      worksheetPath: opaquePath,
      name: 'Rust supplied label',
      databaseId: 'database-1',
      chartType: 'scatter',
      revision: 7,
    }]);

    expect(resolveTabDisplayName({ kind: 'worksheet', id: opaquePath }, 'fallback'))
      .toBe('Rust supplied label');
  });

  it('loads and installs a worksheet by opaque worksheetPath', async () => {
    const opaquePath = 'worksheets/Path With Spaces.yssbi-worksheet';
    vi.spyOn(WorksheetService, 'loadWorksheet').mockResolvedValue(worksheet(7));

    await expect(management.ensureWorksheetLoaded(opaquePath)).resolves.toEqual(worksheet(7));

    expect(WorksheetService.loadWorksheet).toHaveBeenCalledWith(projectA, opaquePath);
    expect(useWorksheetStore.getState().documents[opaquePath]).toEqual(worksheet(7));
  });

  it('duplicates with authoritative revision and opens only the Rust lifecycle destination', async () => {
    const destinationPath = 'worksheets/Destination Opaque.yssbi-worksheet';
    useWorksheetStore.getState().setIndex([{
      worksheetPath,
      name: 'Source label',
      databaseId: 'database-1',
      chartType: 'scatter',
      revision: 7,
    }]);
    vi.spyOn(WorksheetService, 'loadWorksheet').mockResolvedValue(worksheet());
    vi.spyOn(WorksheetService, 'duplicateWorksheet').mockImplementation(
      async (_projectInstanceId, operationId) => ({
        ...mutation(worksheet(), 1, operationId),
        deltas: [worksheetLifecycleDelta(
          worksheet(),
          false,
          destinationPath,
          'Rust allocated copy label',
          operationId,
        )],
      }),
    );

    await management.duplicateWorksheet(worksheetPath);

    expect(WorksheetService.duplicateWorksheet).toHaveBeenCalledWith(
      projectA,
      expect.any(String),
      worksheetPath,
      7,
    );
    expect(openWorksheet).toHaveBeenCalledWith(destinationPath, 'Rust allocated copy label');
  });

  it('restores the prior destination document when create publication rejects', async () => {
    const destinationPath = 'worksheets/Create Rollback.yssbi-worksheet';
    const prior = worksheet(5);
    const staged = worksheet(0);
    useWorksheetStore.getState().upsertDocument(destinationPath, prior);
    const priorDocumentState = useDocumentStateStore.getState().documents[
      resourceKey({ id: destinationPath, kind: 'worksheet' })
    ];
    vi.spyOn(WorksheetService, 'loadWorksheet').mockResolvedValue(staged);
    vi.spyOn(WorksheetService, 'createWorksheet').mockImplementation(
      async (_projectInstanceId, operationId) => ({
        ...mutation(staged, 1, operationId),
        deltas: [worksheetLifecycleDelta(
          staged,
          false,
          destinationPath,
          'Create rollback',
          operationId,
        )],
      }),
    );
    vi.spyOn(projectPublicationCoordinator, 'submit').mockRejectedValue(
      new Error('publication recovery failed'),
    );

    await management.addWorksheet();

    expect(useWorksheetStore.getState().documents[destinationPath]).toBe(prior);
    expect(useDocumentStateStore.getState().documents[
      resourceKey({ id: destinationPath, kind: 'worksheet' })
    ]).toBe(priorDocumentState);
    expect(openWorksheet).not.toHaveBeenCalled();
  });

  it('removes the exact staged duplicate document when publication rejects', async () => {
    const destinationPath = 'worksheets/Duplicate Rollback.yssbi-worksheet';
    const staged = worksheet(0);
    useWorksheetStore.getState().setIndex([{
      worksheetPath,
      name: 'Source label',
      databaseId: 'database-1',
      chartType: 'scatter',
      revision: 7,
    }]);
    vi.spyOn(WorksheetService, 'loadWorksheet').mockResolvedValue(staged);
    vi.spyOn(WorksheetService, 'duplicateWorksheet').mockImplementation(
      async (_projectInstanceId, operationId) => ({
        ...mutation(staged, 1, operationId),
        deltas: [worksheetLifecycleDelta(
          staged,
          false,
          destinationPath,
          'Duplicate rollback',
          operationId,
        )],
      }),
    );
    vi.spyOn(projectPublicationCoordinator, 'submit').mockRejectedValue(
      new Error('publication recovery failed'),
    );

    await management.duplicateWorksheet(worksheetPath);

    expect(useWorksheetStore.getState().documents[destinationPath]).toBeUndefined();
    expect(useDocumentStateStore.getState().documents[
      resourceKey({ id: destinationPath, kind: 'worksheet' })
    ]).toBeUndefined();
    expect(openWorksheet).not.toHaveBeenCalled();
  });

  it('preserves a newer concurrent duplicate document when publication rejects', async () => {
    const destinationPath = 'worksheets/Concurrent Destination.yssbi-worksheet';
    const staged = worksheet(0);
    const concurrent = worksheet(2);
    useWorksheetStore.getState().setIndex([{
      worksheetPath,
      name: 'Source label',
      databaseId: 'database-1',
      chartType: 'scatter',
      revision: 7,
    }]);
    vi.spyOn(WorksheetService, 'loadWorksheet').mockResolvedValue(staged);
    vi.spyOn(WorksheetService, 'duplicateWorksheet').mockImplementation(
      async (_projectInstanceId, operationId) => ({
        ...mutation(staged, 1, operationId),
        deltas: [worksheetLifecycleDelta(
          staged,
          false,
          destinationPath,
          'Concurrent destination',
          operationId,
        )],
      }),
    );
    vi.spyOn(projectPublicationCoordinator, 'submit').mockImplementation(async () => {
      useWorksheetStore.getState().upsertDocument(destinationPath, concurrent);
      throw new Error('publication recovery failed');
    });

    await management.duplicateWorksheet(worksheetPath);

    expect(useWorksheetStore.getState().documents[destinationPath]).toBe(concurrent);
  });

  it('gives a stale create completion zero follow-up effects in the replacement project', async () => {
    const request = deferred<ResourceMutationResultDto>();
    vi.spyOn(WorksheetService, 'createWorksheet').mockReturnValue(request.promise);
    const completion = management.addWorksheet();
    await vi.waitFor(() => expect(WorksheetService.createWorksheet).toHaveBeenCalled());
    replaceProject();
    request.resolve(mutation(worksheet()));
    await completion;

    expect(commitFileFirstResourceIndex).not.toHaveBeenCalled();
    expect(openWorksheet).not.toHaveBeenCalled();
    expect(useWorksheetStore.getState().documents).toEqual({});
  });

  it('does not install a stale load completion into the replacement project', async () => {
    const request = deferred<WorksheetDocument>();
    vi.spyOn(WorksheetService, 'loadWorksheet').mockReturnValue(request.promise);

    const completion = management.ensureWorksheetLoaded('worksheet-1');
    await vi.waitFor(() => expect(WorksheetService.loadWorksheet).toHaveBeenCalled());
    replaceProject();
    request.resolve(worksheet());

    await expect(completion).resolves.toBeNull();
    expect(useWorksheetStore.getState().documents).toEqual({});
  });


  it('keeps the tab and document until successful remove publication settles', async () => {
    const request = deferred<ResourceMutationResultDto>();
    useWorksheetStore.getState().setIndex([{
      worksheetPath,
      name: 'Rust supplied label',
      databaseId: 'database-1',
      chartType: 'scatter',
      revision: 0,
    }]);
    useResourceStore.setState({
      resources: {
        [`yssbi://worksheet/${worksheetPath}`]: {
          id: worksheetPath,
          kind: 'worksheet',
          name: 'Rust supplied label',
          uri: `yssbi://worksheet/${worksheetPath}`,
          revision: 0,
          exists: true,
          loaded: true,
          hasDirtyDocument: false,
          hasStaleDocument: false,
          hasConflictDocument: false,
        },
      },
      graphOrder: [],
    });
    useWorksheetStore.getState().upsertDocument(worksheetPath, worksheet());
    useEditorTabStore.setState({ registry: {}, placements: {} });
    useEditorTabStore.getState().initGroupPlacement(
      'editor',
      [buildWorksheetLayoutTab(worksheetPath)],
      worksheetPath,
    );
    vi.spyOn(WorksheetService, 'removeWorksheet').mockReturnValue(request.promise);

    const completion = performWorksheetDelete(worksheetPath);
    await vi.waitFor(() => expect(WorksheetService.removeWorksheet).toHaveBeenCalled());
    expect(useEditorTabStore.getState().resolveTab(worksheetPath)).toBeDefined();
    expect(useWorksheetStore.getState().documents[worksheetPath]).toBeDefined();

    const deleted = worksheet(0);
    request.resolve({
      ...mutation(deleted),
      deltas: [worksheetLifecycleDelta(deleted, true)],
    });
    await expect(completion).resolves.toBe(true);

    expect(useEditorTabStore.getState().resolveTab(worksheetPath)).toBeNull();
    expect(useWorksheetStore.getState().documents[worksheetPath]).toBeUndefined();
  });

  it('preserves the tab document and detail focus when remove fails', async () => {
    useWorksheetStore.getState().upsertDocument(worksheetPath, worksheet());
    useEditorStore.getState().setDetailFocus({ kind: 'worksheet', worksheetPath });
    useEditorTabStore.setState({ registry: {}, placements: {} });
    useEditorTabStore.getState().initGroupPlacement(
      'editor',
      [buildWorksheetLayoutTab(worksheetPath)],
      worksheetPath,
    );
    vi.spyOn(WorksheetService, 'removeWorksheet').mockRejectedValue(new Error('remove failed'));

    await expect(performWorksheetDelete(worksheetPath)).rejects.toThrow('remove failed');

    expect(useEditorTabStore.getState().resolveTab(worksheetPath)).toBeDefined();
    expect(useWorksheetStore.getState().documents[worksheetPath]).toEqual(worksheet());
    expect(useEditorStore.getState().detailFocus).toEqual({
      kind: 'worksheet',
      worksheetPath,
    });
  });

  it('gives a stale remove completion zero effects in the replacement project', async () => {
    const request = deferred<ResourceMutationResultDto>();
    useWorksheetStore.getState().upsertDocument(worksheetPath, worksheet());
    vi.spyOn(WorksheetService, 'removeWorksheet').mockReturnValue(request.promise);

    const completion = performWorksheetDelete(worksheetPath);
    await vi.waitFor(() => expect(WorksheetService.removeWorksheet).toHaveBeenCalled());
    replaceProject();
    const deleted = worksheet(1);
    request.resolve({
      ...mutation(deleted),
      deltas: [worksheetLifecycleDelta(deleted, true)],
    });

    await expect(completion).resolves.toBe(false);
    expect(projectPublicationCoordinator.getSnapshotForTests()).toMatchObject({
      projectInstanceId: 'project-b',
      appliedRevision: 0,
    });
    expect(useWorksheetStore.getState().documents).toEqual({});
  });
});
