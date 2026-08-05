import { beforeEach, describe, expect, it } from 'vitest';
import { useGraphDataStore } from '@/features/core/dataStore';
import { useGraphMetaStore } from '@/features/core/dataStore/graphMetaStore';
import { useVariableStore } from '@/features/core/dataStore/variableStore';
import { useEditorStore } from '@/features/core/editor/stores/useEditorStore';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import {
  cascadeGraphPathReferences,
  cascadeSubGraphPathInLoadedGraphs,
} from './cascadeGraphPathReferences';

describe('cascadeGraphPathReferences', () => {
  beforeEach(() => {
    useGraphDataStore.setState({ graphEntities: {} });
    useGraphMetaStore.setState({ graphs: {} });
    useVariableStore.setState({ variables: {} });
    useEditorStore.setState({
      detailFocus: null,
      variablesGraphScopePath: null,
    });
  });

  it('updates Call Function subGraphPath in caller graphs', () => {
    const from = 'functions/Old.yssbi-function';
    const to = 'functions/New.yssbi-function';

    const graphPath = 'events/Caller.yssbi-event';
    const fixture = makeEditorProjectionFixture({
      graphPath,
      nodeId: 'call-1',
      nodeTypeId: 'yssbi.project.function.call',
      title: 'Localized call title',
    });
    useGraphDataStore.getState().replaceProjection(graphPath, fixture.projection, 1);
    useGraphDataStore.setState((state) => ({
      graphEntities: {
        ...state.graphEntities,
        [graphPath]: {
          ...state.graphEntities[graphPath],
          nodes: {
            ...state.graphEntities[graphPath].nodes,
            'call-1': { ...state.graphEntities[graphPath].nodes['call-1'], subGraphPath: from },
            'legacy-call': {
              ...state.graphEntities[graphPath].nodes['call-1'],
              id: 'legacy-call',
              nodeType: 'Functions:Call Function',
              subGraphPath: from,
              title: 'Legacy call label',
            },
          },
        },
      },
    }));

    cascadeSubGraphPathInLoadedGraphs(from, to);

    const node =
      useGraphDataStore.getState().graphEntities['events/Caller.yssbi-event']?.nodes['call-1'];
    expect(node?.subGraphPath).toBe(to);
    expect(useGraphDataStore.getState().graphEntities[graphPath]?.nodes['legacy-call']?.subGraphPath)
      .toBe(from);
  });

  it('remaps graph meta, variable scope, and editor focus', () => {
    const from = 'functions/Old.yssbi-function';
    const to = 'functions/New.yssbi-function';

    useGraphMetaStore.setState({
      graphs: {
        [from]: { path: from, name: 'Old', type: 'function' },
      },
    });
    useVariableStore.setState({
      variables: {
        'var-1': {
          id: 'var-1',
          name: 'x',
          dataType: { kind: 'Int64' },
          dataValue: { kind: 'Int64', value: 0 },
          description: '',
          scope: { type: 'function', functionPath: from },
          tags: [],
        },
      },
    });
    useEditorStore.setState({
      detailFocus: { kind: 'function', path: from },
      variablesGraphScopePath: from,
    });

    cascadeGraphPathReferences(from, to);

    expect(useGraphMetaStore.getState().graphs[to]?.name).toBe('Old');
    expect(useGraphMetaStore.getState().graphs[from]).toBeUndefined();
    expect(useVariableStore.getState().variables['var-1']?.scope).toEqual({
      type: 'function',
      functionPath: to,
    });
    expect(useEditorStore.getState().detailFocus).toEqual({ kind: 'function', path: to });
    expect(useEditorStore.getState().variablesGraphScopePath).toBe(to);
  });
});
