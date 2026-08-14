import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

const productionFiles = [
  'src/features/core/sync/handlers/ProjectMutationEventHandler.ts',
  'src/features/application/editorMutation/functionSignatureCoordinator.ts',
  'src/features/application/editorMutation/historyCoordinator.ts',
  'src/features/application/resource/resourceActions.ts',
  'src/features/application/editor/closeEditorTab.ts',
  'src/features/application/editor/useWorksheetManagement.ts',
  'src/features/application/dataManagement/variableActions.ts',
  'src/features/core/dataStore/projectIOStore.ts',
  'src/features/application/initialization/registerCoreApplicationPorts.ts',
];

describe('project publication integration boundary', () => {
  it('leaves one publication owner and no legacy applier or move path', () => {
    const sources = productionFiles.map((path) =>
      [path, readFileSync(resolve(path), 'utf8')] as const,
    );
    const joined = sources.map(([path, source]) => `${path}\n${source}`).join('\n');

    expect(joined).not.toMatch(/applyResourceMutationResultWithMoves/);
    expect(joined).not.toMatch(/validateAndApplyResourceMutationResult/);
    expect(joined).not.toMatch(/migrateGraphResourcePath/);
    expect(joined).not.toMatch(/setResourceMutationProjectInstanceId/);
    expect(joined).not.toMatch(/resetResourceMutationPublicationState/);
    expect(sources[0][1]).toContain('syncApplicationEventPort().resourceMutationCommitted');
    for (const [path, source] of sources.slice(1, 7)) {
      expect(source, path).toContain('projectPublicationCoordinator.submit');
    }
    expect(sources[7][1]).toContain('projectIOApplicationPort().startPublication');
    expect(sources[7][1]).toContain('projectIOApplicationPort().acceptProjectActivation');
    expect(sources[7][1]).not.toMatch(/latestPublicationRevision|publicationDrain|authoritativeGapRecovery/);
    expect(sources[8][1]).toContain('projectPublicationCoordinator.startProject');
    expect(sources[8][1]).toContain('projectPublicationCoordinator.submit');
  });
});
