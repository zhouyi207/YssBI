import { beforeEach, describe, expect, it } from 'vitest';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { useGraphMetaStore } from '@/features/core/dataStore/graphMetaStore';
import { useVariableStore } from '@/features/core/dataStore/variableStore';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import { useViewportStore } from '@/features/core/viewport';
import { viewportScopeKey } from '@/features/core/viewport/viewportScope';
import {
  buildGraphResourceMeta,
  markResourceDirty,
  markResourceLoaded,
  markResourceStale,
  resourceKey,
  useDocumentStateStore,
  useResourceStore,
} from '@/features/core/resource';
import { prepareGraphResourceMove } from './projectPublicationMovePlan';
import {
  commitPreparedPublication,
  prepareSynchronousPublicationCommit,
} from './resourceMutationResult';
import type { ResourceMutationResultDto } from '@/shared/types/dto/editorMutation';

const from = 'events/Old.yssbi-event';
const to = 'events/New.yssbi-event';

function snapshotPathOwnedState() {
  return structuredClone({
    graphEntities: useGraphDataStore.getState().graphEntities,
    graphs: useGraphMetaStore.getState().graphs,
    variables: useVariableStore.getState().variables,
    resources: useResourceStore.getState().resources,
    graphOrder: useResourceStore.getState().graphOrder,
    documents: useDocumentStateStore.getState().documents,
    focusedSession: useGraphSessionStore.getState().focusedSession,
    tabs: useEditorTabStore.getState().snapshotMemento(),
    viewports: useViewportStore.getState().viewports,
  });
}

function seedSource() {
  useResourceStore.getState().setSnapshot({
    resources: [buildGraphResourceMeta('event', from, 'Old')],
    graphOrder: [from],
  });
  markResourceLoaded({ id: from, kind: 'event' });
  markResourceDirty({ id: from, kind: 'event' }, true);
  markResourceStale({ id: from, kind: 'event' }, true);
  useDocumentStateStore.getState().patchDocument(resourceKey({ id: from, kind: 'event' }), {
    conflict: true,
  });
  useResourceStore.getState().patchResource({ id: from, kind: 'event' }, {
    hasConflictDocument: true,
  });
  useGraphMetaStore.setState({
    graphs: { [from]: { path: from, name: 'Old', type: 'event' } },
  });
  useVariableStore.setState({
    variables: {
      scoped: {
        id: 'scoped',
        name: 'Scoped',
        dataType: { kind: 'Int64' },
        dataValue: { kind: 'Int64', value: 1 },
        description: '',
        scope: { type: 'event', eventPath: from },
        tags: [],
      },
    },
  });
  useGraphSessionStore.getState().setFocusedSession('editor', from);
  useEditorTabStore.getState().initGroupPlacement('editor', [{
    id: from,
    component: 'GraphEditor',
    type: 'event',
  }], from);
  useViewportStore.getState().setViewport({ groupId: 'editor', graphPath: from }, {
    x: 12,
    y: 24,
    scale: 1.5,
  });
  useGraphDataStore.getState().replaceProjection(
    from,
    makeEditorProjectionFixture({ graphPath: from, title: 'Old' }).projection,
    1,
  );
}

describe('project publication graph resource move plan', () => {
  beforeEach(() => {
    useGraphDataStore.setState({ graphEntities: {} });
    useGraphMetaStore.setState({ graphs: {} });
    useVariableStore.setState({ variables: {} });
    useResourceStore.getState().clear();
    useDocumentStateStore.getState().clear();
    useGraphSessionStore.getState().reset();
    useEditorTabStore.setState({ registry: {}, placements: {} });
    useViewportStore.getState().clear();
    seedSource();
  });

  it('prepares the complete move without mutating path-owned stores', () => {
    useGraphDataStore.setState((state) => ({
      graphEntities: {
        ...state.graphEntities,
        [from]: {
          ...state.graphEntities[from],
          nodes: {
            ...state.graphEntities[from].nodes,
            'stable-call': {
              ...state.graphEntities[from].nodes['local-node'],
              id: 'stable-call',
              nodeType: 'yssbi.project.function.call',
              subGraphPath: from,
              title: 'Localized call title',
            },
            'legacy-call': {
              ...state.graphEntities[from].nodes['local-node'],
              id: 'legacy-call',
              nodeType: 'Functions:Call Function',
              subGraphPath: from,
              title: 'Legacy call label',
            },
          },
        },
      },
    }));
    const before = snapshotPathOwnedState();
    const plan = prepareGraphResourceMove({
      from,
      to,
      kind: 'event',
      name: 'New',
    }, true);

    expect(snapshotPathOwnedState()).toEqual(before);
    expect(plan).toMatchObject({
      from,
      to,
      kind: 'event',
      name: 'New',
      hasAuthoritativeDestinationReplacement: true,
    });
  });

  it('commits the prepared move synchronously and preserves document flags and owners', () => {
    const destination = makeEditorProjectionFixture({ graphPath: to, title: 'New' }).projection;
    const move = { from, to, kind: 'event' as const, name: 'New' };
    const preparedMove = prepareGraphResourceMove(move, true);
    const result: ResourceMutationResultDto = {
      operationId: '00000000-0000-0000-0000-000000000903',
      projectInstanceId: '00000000-0000-0000-0000-000000000901',
      publicationRevision: 1,
      moves: [move],
      deltas: [{
        resource: { kind: 'graph', key: to },
        fromRevision: 0,
        toRevision: 1,
        causedBy: null,
        payload: { kind: 'graph_resource_move', patch: { from, to } },
      }],
      projectionReplacements: [{ graphPath: to, projection: destination }],
      projectionStatus: { status: 'complete', expectedGraphPaths: [to] },
      history: { canUndo: true, canRedo: false },
    };
    const plan = prepareSynchronousPublicationCommit(result, {
      projectInstanceId: result.projectInstanceId,
      epoch: 1,
      fingerprint: 'loaded-move',
      affectedGraphPaths: new Set([from, to]),
      moves: [preparedMove],
    });

    expect(commitPreparedPublication(plan)).toBeUndefined();

    expect(useGraphDataStore.getState().graphEntities[from]).toBeUndefined();
    expect(useGraphDataStore.getState().graphEntities[to]?.basis.graphPath).toBe(to);
    expect(useGraphMetaStore.getState().graphs).toEqual({
      [to]: { path: to, name: 'New', type: 'event' },
    });
    expect(useVariableStore.getState().variables.scoped.scope).toEqual({
      type: 'event',
      eventPath: from,
    });
    expect(useGraphSessionStore.getState().focusedSession).toEqual({
      groupId: 'editor',
      graphPath: to,
    });
    expect(useEditorTabStore.getState().getPlacement('editor')).toMatchObject({
      tabIds: [to],
      activeTabId: to,
    });
    expect(useViewportStore.getState().viewports).toEqual({
      [viewportScopeKey({ groupId: 'editor', graphPath: to })]: {
        x: 12,
        y: 24,
        scale: 1.5,
      },
    });
    expect(useResourceStore.getState().graphOrder).toEqual([to]);
    expect(useResourceStore.getState().resources).toMatchObject({
      [resourceKey({ id: to, kind: 'event' })]: {
        id: to,
        name: 'New',
        loaded: true,
        hasDirtyDocument: true,
        hasStaleDocument: true,
        hasConflictDocument: true,
      },
    });
    expect(useDocumentStateStore.getState().documents).toMatchObject({
      [resourceKey({ id: to, kind: 'event' })]: {
        loaded: true,
        dirty: true,
        stale: true,
        conflict: true,
      },
    });
  });

  it('installs an authoritative caller replacement exactly instead of scanning local call targets', () => {
    const caller = 'events/Caller.yssbi-event';
    const localCaller = makeEditorProjectionFixture({
      graphPath: caller,
      sourceRevision: 1,
      nodeId: 'stable-call',
      nodeTypeId: 'yssbi.project.function.call',
      title: 'Locally inferred caller',
    }).projection;
    useGraphDataStore.getState().replaceProjection(caller, localCaller, 1);
    useGraphDataStore.setState((state) => ({
      graphEntities: {
        ...state.graphEntities,
        [caller]: {
          ...state.graphEntities[caller],
          nodes: {
            ...state.graphEntities[caller].nodes,
            'stable-call': {
              ...state.graphEntities[caller].nodes['stable-call'],
              subGraphPath: from,
            },
          },
        },
      },
    }));
    const destination = makeEditorProjectionFixture({
      graphPath: to,
      sourceRevision: 1,
      title: 'Authoritative destination',
    }).projection;
    const authoritativeCaller = makeEditorProjectionFixture({
      graphPath: caller,
      sourceRevision: 2,
      nodeId: 'stable-call',
      nodeTypeId: 'yssbi.project.function.call',
      title: 'Authoritative caller replacement',
    }).projection;
    const move = { from, to, kind: 'event' as const, name: 'New' };
    const preparedMove = prepareGraphResourceMove(move, true);
    const result: ResourceMutationResultDto = {
      operationId: '00000000-0000-0000-0000-000000000904',
      projectInstanceId: '00000000-0000-0000-0000-000000000901',
      publicationRevision: 1,
      moves: [move],
      deltas: [{
        resource: { kind: 'graph', key: to },
        fromRevision: 0,
        toRevision: 1,
        causedBy: null,
        payload: { kind: 'graph_resource_move', patch: { from, to } },
      }],
      projectionReplacements: [
        { graphPath: to, projection: destination },
        { graphPath: caller, projection: authoritativeCaller },
      ],
      projectionStatus: { status: 'complete', expectedGraphPaths: [to, caller] },
      history: { canUndo: true, canRedo: false },
    };

    const plan = prepareSynchronousPublicationCommit(result, {
      projectInstanceId: result.projectInstanceId,
      epoch: 1,
      fingerprint: 'authoritative-move-replacements',
      affectedGraphPaths: new Set([from, to, caller]),
      moves: [preparedMove],
    });
    commitPreparedPublication(plan);

    const installed = useGraphDataStore.getState().graphEntities[caller]?.nodes['stable-call'];
    expect(installed).toMatchObject({
      id: 'stable-call',
      title: 'Authoritative caller replacement',
    });
    expect(installed).not.toHaveProperty('subGraphPath');
  });

  it('rejects replacement residency mismatches and conflicting destinations before commit', () => {
    expect(() => prepareGraphResourceMove({
      from,
      to,
      kind: 'event',
      name: 'New',
    }, false)).toThrow('replacement disagrees with source loaded ownership');

    useResourceStore.getState().upsertResource(buildGraphResourceMeta('event', to, 'Existing'));
    expect(() => prepareGraphResourceMove({
      from,
      to,
      kind: 'event',
      name: 'New',
    }, true)).toThrow('destination resource');
  });
});
