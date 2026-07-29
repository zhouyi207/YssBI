// @vitest-environment happy-dom

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import { buildWorksheetLayoutTab } from '@/features/core/layout/layoutTabModel';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { useDocumentStateStore, useResourceStore } from '@/features/core/resource';
import { uiStore } from '@/features/core/ui/UIStore';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import {
  WorksheetService,
  type WorksheetMutationResultDto,
} from '@/services/worksheet/worksheetService';
import type { WorksheetDocument } from '@/shared/types/domain/worksheet';
import { performWorksheetDelete } from './closeEditorTab';
import { useWorksheetManagement } from './useWorksheetManagement';
import { useProjectOperations } from './useProjectOperations';
import { commitFileFirstResourceIndex } from '@/features/application/resource/resourceActions';

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
vi.mock('@/features/application/execution/openInspectableSource', () => ({
  openWindowInspectableSource: vi.fn(),
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

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((settle) => { resolve = settle; });
  return { promise, resolve };
}

function worksheet(revision = 0): WorksheetDocument {
  return {
    schemaVersion: 3,
    revision,
    id: 'worksheet-1',
    name: 'Worksheet',
    databaseId: 'database-1',
    chartType: 'scatter',
    encodings: { x: 'x', y: 'y' },
  };
}

function mutation(document: WorksheetDocument, publicationRevision = 1): WorksheetMutationResultDto {
  return {
    operationId: '00000000-0000-0000-0000-000000000501',
    document,
    result: {
      operationId: '00000000-0000-0000-0000-000000000501',
      projectInstanceId: projectA,
      publicationRevision,
      moves: [],
      deltas: [],
      worksheetDeltas: [{ id: document.id, before: null, after: document }],
      projectionReplacements: [],
      projectionStatus: { status: 'complete', expectedGraphPaths: [] },
      history: { canUndo: true, canRedo: false },
    },
  };
}

function replaceProject(): void {
  useProjectIOStore.setState({ projectInstanceId: 'project-b' });
  projectPublicationCoordinator.startProject('project-b', 0);
  useWorksheetStore.getState().clear();
}

describe('worksheet command lifecycle guards', () => {
  let host: HTMLDivElement;
  let root: Root;
  let management!: ReturnType<typeof useWorksheetManagement>;
  let operations!: ReturnType<typeof useProjectOperations>;
  const openWorksheet = vi.fn(async () => undefined);

  beforeEach(() => {
    vi.restoreAllMocks();
    vi.mocked(commitFileFirstResourceIndex).mockResolvedValue(true);
    useWorksheetStore.getState().clear();
    useDocumentStateStore.getState().clear();
    useResourceStore.getState().clear();
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

  it('gives a stale create completion zero follow-up effects in the replacement project', async () => {
    const request = deferred<WorksheetMutationResultDto>();
    vi.spyOn(WorksheetService, 'createWorksheet').mockReturnValue(request.promise);
    const toast = vi.spyOn(uiStore, 'showToast');

    const completion = management.addWorksheet();
    await vi.waitFor(() => expect(WorksheetService.createWorksheet).toHaveBeenCalled());
    replaceProject();
    request.resolve(mutation(worksheet()));
    await completion;

    expect(commitFileFirstResourceIndex).not.toHaveBeenCalled();
    expect(openWorksheet).not.toHaveBeenCalled();
    expect(toast).not.toHaveBeenCalled();
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

  it('does not toast success when the active worksheet save basis became stale', async () => {
    useLayoutStore.setState({
      rootId: 'root',
      nodes: {
        root: { id: 'root', type: 'row', parentId: null, children: ['editor'] },
        editor: {
          id: 'editor',
          type: 'component',
          parentId: 'root',
          data: { component: 'GraphEditor' },
        },
      },
      activeEditorGroupId: 'editor',
    });
    useEditorTabStore.setState({ registry: {}, placements: {} });
    useEditorTabStore.getState().initGroupPlacement(
      'editor',
      [buildWorksheetLayoutTab('worksheet-1')],
      'worksheet-1',
    );
    function OperationsHarness() {
      operations = useProjectOperations();
      return null;
    }
    act(() => root.render(<OperationsHarness />));
    vi.spyOn(useWorksheetStore.getState(), 'saveDocument').mockResolvedValue(false);
    const toast = vi.spyOn(uiStore, 'showToast');

    await operations.saveGraph();

    expect(toast).not.toHaveBeenCalled();
  });

  it('gives a stale delete completion zero effects in the replacement project', async () => {
    const request = deferred<WorksheetMutationResultDto>();
    vi.spyOn(WorksheetService, 'deleteWorksheet').mockReturnValue(request.promise);

    const completion = performWorksheetDelete('worksheet-1');
    await vi.waitFor(() => expect(WorksheetService.deleteWorksheet).toHaveBeenCalled());
    replaceProject();
    const deleted = worksheet(1);
    request.resolve({
      ...mutation(deleted),
      result: {
        ...mutation(deleted).result,
        worksheetDeltas: [{ id: deleted.id, before: deleted, after: null }],
      },
    });

    await expect(completion).resolves.toBe(false);
    expect(projectPublicationCoordinator.getSnapshotForTests()).toMatchObject({
      projectInstanceId: 'project-b',
      appliedRevision: 0,
    });
    expect(useWorksheetStore.getState().documents).toEqual({});
  });
});
