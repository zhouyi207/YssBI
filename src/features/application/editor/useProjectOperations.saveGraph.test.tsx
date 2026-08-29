// @vitest-environment happy-dom

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { EditorCommandTarget } from './editorCommandFocus';

const target: EditorCommandTarget = Object.freeze({
  panelInstanceId: 'panel-main',
  groupId: 'group-main',
  resourceRef: 'events/Main.yssbi-event',
  resourceKind: 'event',
});

const mocks = vi.hoisted(() => ({
  targetCurrent: true,
  resolveActiveProjectPath: vi.fn(async () => 'D:/projects/demo'),
  captureSettledGraphSaveCommandContext: vi.fn(),
  isGraphSaveCommandRevisionCurrent: vi.fn(() => true),
  saveProjectGraph: vi.fn(async () => undefined),
  saveWorksheet: vi.fn(async () => true),
  markResourceDirty: vi.fn(),
  warnCallFunctionIssuesBeforeSave: vi.fn(),
  showBlockingMessage: vi.fn(),
  showBlockingIpcError: vi.fn(),
}));

vi.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: () => undefined },
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('@/features/core/dataStore', () => ({
  loadActivatedProject: vi.fn(),
  resolveActiveProjectPath: mocks.resolveActiveProjectPath,
}));

vi.mock('@/features/application/project/projectSession', () => ({
  resolveActiveProjectPath: mocks.resolveActiveProjectPath,
}));

vi.mock('@/features/application/worksheet/saveWorksheetDocument', () => ({
  saveWorksheetDocument: mocks.saveWorksheet,
}));

vi.mock('@/features/core/layout/layoutTabQueries', () => ({
  resolveEditorGroupId: () => 'group-later',
  getActiveLayoutTab: () => ({
    activeTabId: 'events/Later.yssbi-event',
    tab: {
      id: 'events/Later.yssbi-event',
      type: 'event',
      component: 'GraphEditor',
    },
  }),
}));

vi.mock('@/features/core/worksheet/worksheetStore', () => ({
  useWorksheetStore: {
    getState: () => ({ saveDocument: mocks.saveWorksheet }),
  },
}));

vi.mock('@/features/core/resource', () => ({
  markResourceDirty: mocks.markResourceDirty,
}));

vi.mock('@/services/graph/graphService', () => ({
  GraphService: {
    saveProjectGraph: mocks.saveProjectGraph,
  },
}));

vi.mock('@/features/application/projectCommandContext', () => ({
  captureSettledGraphSaveCommandContext: mocks.captureSettledGraphSaveCommandContext,
  isGraphSaveCommandRevisionCurrent: mocks.isGraphSaveCommandRevisionCurrent,
}));

vi.mock('@/features/application/graphDiagnostics/warnCallFunctionIssues', () => ({
  warnCallFunctionIssuesBeforeSave: mocks.warnCallFunctionIssuesBeforeSave,
}));

vi.mock('@/features/application/execution/openInspectableResult', () => ({
  openInspectableResult: vi.fn(async () => true),
}));

vi.mock('./editorCommandFocus', () => ({
  captureActiveEditorCommandTarget: () => target,
  isEditorCommandTargetCurrent: () => mocks.targetCurrent,
}));

vi.mock('./blockingErrorDialog', () => ({
  showBlockingMessage: mocks.showBlockingMessage,
  showBlockingIpcError: mocks.showBlockingIpcError,
}));

vi.mock('@/features/core/execution', () => ({
  useExecutionStore: { getState: () => ({}) },
  getExecutionEventGraph: vi.fn(),
  resolveExecutionGraphPath: vi.fn(),
  graphHasClearableArtifacts: vi.fn(),
}));

vi.mock('@/features/application/observability/appLogger', () => ({
  logger: {
    app: { error: vi.fn() },
    exec: { info: vi.fn(), error: vi.fn() },
  },
}));

import { useProjectOperations } from './useProjectOperations';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function graphSaveContext() {
  return {
    projectInstanceId: 'project-main',
    projectEpoch: 1,
    publicationRevision: 1,
    expectedRevision: 7,
    operationId: 'save-main',
    operationPendingKey: 'project-main:save-main',
    isCurrent: () => true,
    assertCurrent: () => undefined,
  };
}

describe('useProjectOperations saveGraph target authority', () => {
  let host: HTMLDivElement;
  let root: Root;
  let operations!: ReturnType<typeof useProjectOperations>;

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.targetCurrent = true;
    mocks.resolveActiveProjectPath.mockResolvedValue('D:/projects/demo');
    mocks.captureSettledGraphSaveCommandContext.mockResolvedValue(graphSaveContext());
    mocks.isGraphSaveCommandRevisionCurrent.mockReturnValue(true);
    mocks.saveProjectGraph.mockResolvedValue(undefined);
    mocks.saveWorksheet.mockResolvedValue(true);

    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);

    function Harness() {
      operations = useProjectOperations();
      return null;
    }

    act(() => root.render(<Harness />));
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it('saves the captured target resource instead of a later active layout tab', async () => {
    await act(async () => {
      await operations.saveGraph(target);
    });

    expect(mocks.warnCallFunctionIssuesBeforeSave).toHaveBeenCalledWith(target.resourceRef);
    expect(mocks.captureSettledGraphSaveCommandContext).toHaveBeenCalledWith(
      target.resourceRef,
    );
    expect(mocks.saveProjectGraph).toHaveBeenCalledWith(
      'project-main',
      target.resourceRef,
      7,
      'save-main',
    );
    expect(mocks.markResourceDirty).toHaveBeenCalledWith({
      id: target.resourceRef,
      kind: target.resourceKind,
    }, false);
  });

  it('stops before graph IPC when the target changes while mutations settle', async () => {
    mocks.captureSettledGraphSaveCommandContext.mockImplementationOnce(async () => {
      mocks.targetCurrent = false;
      return graphSaveContext();
    });

    await act(async () => {
      await operations.saveGraph(target);
    });

    expect(mocks.captureSettledGraphSaveCommandContext).toHaveBeenCalledWith(
      target.resourceRef,
    );
    expect(mocks.saveProjectGraph).not.toHaveBeenCalled();
    expect(mocks.markResourceDirty).not.toHaveBeenCalled();
  });

  it('does not clear dirty state when the target changes during graph IPC', async () => {
    mocks.saveProjectGraph.mockImplementationOnce(async () => {
      mocks.targetCurrent = false;
    });

    await act(async () => {
      await operations.saveGraph(target);
    });

    expect(mocks.saveProjectGraph).toHaveBeenCalledOnce();
    expect(mocks.isGraphSaveCommandRevisionCurrent).not.toHaveBeenCalled();
    expect(mocks.markResourceDirty).not.toHaveBeenCalled();
  });

  it('saves a worksheet by its captured target and ignores stale settlement feedback', async () => {
    const worksheetTarget: EditorCommandTarget = Object.freeze({
      panelInstanceId: 'panel-worksheet',
      groupId: 'group-main',
      resourceRef: 'worksheets/Summary.yssbi-worksheet',
      resourceKind: 'worksheet',
    });
    mocks.saveWorksheet.mockImplementationOnce(async () => {
      mocks.targetCurrent = false;
      return false;
    });

    await act(async () => {
      await operations.saveGraph(worksheetTarget);
    });

    expect(mocks.saveWorksheet).toHaveBeenCalledWith(worksheetTarget.resourceRef);
    expect(mocks.captureSettledGraphSaveCommandContext).not.toHaveBeenCalled();
    expect(mocks.saveProjectGraph).not.toHaveBeenCalled();
    expect(mocks.showBlockingMessage).not.toHaveBeenCalled();
  });
});
