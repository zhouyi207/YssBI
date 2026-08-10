import { expect, it } from 'vitest';
import type { ProjectIndexRow } from '@/services/project/projectService';
import { resourceKey } from '@/features/core/resource';
import { prepareProjectRecoveryCommit } from './projectPublicationRecovery';

it('prepares recovery with authoritative function editor projection pins', () => {
  const functionPath = 'functions/Model.yssbi-function';
  const projectInstanceId = '00000000-0000-0000-0000-000000000701';
  const index: ProjectIndexRow = {
    projectInstanceId,
    projectName: 'Recovered',

    exportTime: '',
    publicationRevision: 4,
    history: { canUndo: true, canRedo: false },
    graphs: [{
      path: functionPath,
      name: 'Model',
      type: 'function',
      revision: 6,
      functionRevision: 6,
      functionSignature: { parameters: [], return_type: 'Object' },
      functionEditorProjection: {
        functionRevision: 6,
        inputs: [],
        outputs: [{
          id: 'computed',
          name: 'Computed value',
          dataType: { kind: 'Struct', inner: 'RegressionModel' },
        }],
      },
    }],
    variables: [],
    worksheets: [],
    databases: [],
  };

  const prepared = prepareProjectRecoveryCommit({
    projectInstanceId,
    epoch: 1,
    publicationRevision: 4,
    index,
    projections: new Map(),
    graphPathsLoadedAtStart: new Set(),
    pathRemaps: new Map(),
  });

  expect(prepared.storeState.resources[
    resourceKey({ id: functionPath, kind: 'function' })
  ]).toMatchObject({ revision: 6 });
  expect(prepared.storeState.graphMeta[functionPath]).toMatchObject({
    functionRevision: 6,
    functionSignature: { parameters: [], return_type: 'Object' },
    functionInputs: [],
    functionOutputs: [{
      id: 'computed',
      name: 'Computed value',
      dataType: { kind: 'Struct', inner: 'RegressionModel' },
    }],
  });
});
