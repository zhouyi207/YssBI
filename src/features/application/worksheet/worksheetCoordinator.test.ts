import { beforeEach, describe, expect, it, vi } from 'vitest';

import {
  createWorksheetCoordinator,
  type WorksheetProjectIdentity,
} from './worksheetCoordinator';
import {
  getWorksheetSnapshot,
  type WorksheetCommittedSnapshot,
} from '@/features/core/worksheet/read';
import {
  worksheetProjectionPublication,
} from '@/features/core/worksheet/publication';
import { worksheetUi } from '@/features/core/worksheet/ui';
import type {
  WorksheetDocument,
  WorksheetIndexEntry,
} from '@/shared/types/domain/worksheet';

const WORKSHEET_PATH = 'worksheets/Report.yssbi-worksheet';
const PROJECT_A = 'project-a';
const PROJECT_B = 'project-b';

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function document(
  revision: number,
  chartType: WorksheetDocument['chartType'],
): WorksheetDocument {
  return {
    schemaVersion: 3,
    revision,
    databaseId: 'database-1',
    chartType,
    encodings: { x: 'x', y: 'y' },
  };
}

function indexEntry(
  worksheetPath: string,
  value: WorksheetDocument,
): WorksheetIndexEntry {
  return {
    worksheetPath,
    name: 'Report',
    databaseId: value.databaseId,
    chartType: value.chartType,
    revision: value.revision,
  };
}

function pendingFor(path: string) {
  const records = getWorksheetSnapshot().pendingSaveByPath[path];
  const record = records ? Object.values(records)[0] : undefined;
  expect(record).toBeDefined();
  return record!;
}

describe('Worksheet coordinator staged boundary', () => {
  beforeEach(() => {
    worksheetProjectionPublication.clearForProject(null);
    vi.restoreAllMocks();
  });

  it('publishes only the current project when an older load resolves after replacement', async () => {
    let identity: WorksheetProjectIdentity = {
      projectInstanceId: PROJECT_A,
      epoch: 1,
    };
    const loadA = deferred<WorksheetDocument>();
    const loadB = deferred<WorksheetDocument>();
    const publishIssue = vi.fn();
    const service = {
      loadWorksheet: vi.fn((projectInstanceId: string) =>
        projectInstanceId === PROJECT_A ? loadA.promise : loadB.promise),
      saveWorksheet: vi.fn().mockResolvedValue({ accepted: true }),
    };
    const coordinator = createWorksheetCoordinator({
      captureProjectIdentity: () => identity,
      service,
      publishIssue,
    });

    const completionA = coordinator.load(WORKSHEET_PATH);
    identity = { projectInstanceId: PROJECT_B, epoch: 2 };
    coordinator.resetProject();
    const completionB = coordinator.load(WORKSHEET_PATH);

    loadB.resolve(document(8, 'line'));
    expect(await completionB).toEqual({ status: 'loaded' });

    loadA.resolve(document(7, 'scatter'));
    expect(await completionA).toEqual({ status: 'stale' });
    expect(getWorksheetSnapshot()).toMatchObject({
      documents: {
        [WORKSHEET_PATH]: document(8, 'line'),
      },
      draftsByPath: {},
      dirtyByPath: {},
      pendingSaveByPath: {},
    });
    expect(publishIssue).not.toHaveBeenCalled();
    expect(service.loadWorksheet).toHaveBeenCalledTimes(2);
  });

  it('keeps ordinary acknowledgements separate from matching committed rebases', async () => {
    const identity: WorksheetProjectIdentity = {
      projectInstanceId: PROJECT_A,
      epoch: 1,
    };
    const base = document(3, 'scatter');
    const saved = document(4, 'line');
    const service = {
      loadWorksheet: vi.fn().mockResolvedValue(base),
      saveWorksheet: vi.fn().mockResolvedValue({ accepted: true }),
    };
    const coordinator = createWorksheetCoordinator({
      captureProjectIdentity: () => identity,
      service,
    });
    coordinator.resetProject();
    worksheetProjectionPublication.replaceSnapshot({
      index: [indexEntry(WORKSHEET_PATH, base)],
      documents: { [WORKSHEET_PATH]: base },
    } satisfies WorksheetCommittedSnapshot);
    worksheetUi.updateDraft(WORKSHEET_PATH, { chartType: 'line' });

    await expect(coordinator.save(WORKSHEET_PATH)).resolves.toEqual({
      status: 'acknowledged',
    });
    const acknowledged = pendingFor(WORKSHEET_PATH);
    expect(acknowledged.status).toBe('acknowledged');
    expect(getWorksheetSnapshot().draftsByPath[WORKSHEET_PATH]).toMatchObject({
      chartType: 'line',
    });
    expect(getWorksheetSnapshot().dirtyByPath[WORKSHEET_PATH]).toBe(true);

    const committed = {
      ...saved,
      encodings: { x: 'x', y: 'y' },
    };
    expect(coordinator.acceptCommittedDocument(
      WORKSHEET_PATH,
      committed,
      acknowledged,
    )).toBe('rebased');
    expect(getWorksheetSnapshot().documents[WORKSHEET_PATH]).toEqual(committed);
    expect(getWorksheetSnapshot().draftsByPath[WORKSHEET_PATH]).toBeUndefined();
    expect(getWorksheetSnapshot().dirtyByPath[WORKSHEET_PATH]).toBe(false);
    expect(getWorksheetSnapshot().pendingSaveByPath[WORKSHEET_PATH]).toBeUndefined();

    worksheetUi.updateDraft(WORKSHEET_PATH, { chartType: 'scatter' });
    await expect(coordinator.save(WORKSHEET_PATH)).resolves.toEqual({
      status: 'acknowledged',
    });
    const secondSave = pendingFor(WORKSHEET_PATH);
    worksheetUi.updateDraft(WORKSHEET_PATH, { chartType: 'histogram' });

    expect(coordinator.acceptCommittedDocument(
      WORKSHEET_PATH,
      document(5, 'scatter'),
      secondSave,
    )).toBe('draft-changed');
    expect(getWorksheetSnapshot().documents[WORKSHEET_PATH]?.chartType).toBe('scatter');
    expect(getWorksheetSnapshot().draftsByPath[WORKSHEET_PATH]?.chartType).toBe('histogram');
    expect(getWorksheetSnapshot().dirtyByPath[WORKSHEET_PATH]).toBe(true);
    expect(getWorksheetSnapshot().pendingSaveByPath[WORKSHEET_PATH]).toBeUndefined();
  });
});
