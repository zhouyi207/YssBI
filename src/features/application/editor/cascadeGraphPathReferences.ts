import { useGraphDataStore, useGraphMetaStore } from '@/features/core/dataStore';
import { useEditorStore } from '@/features/core/editor/stores/useEditorStore';
import { useVariableStore } from '@/features/core/dataStore/variableStore';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import { remapEditorViewStateGraphPath } from '@/features/core/viewport/editorViewStateMemento';
import { remapGraphViewport } from '@/features/core/viewport/useViewportStore';
import { isCallFunctionNodeType } from '@/features/domain/nodeCatalog';
import { normalizeGraphResourcePath } from '@/shared/types/domain/graphResourcePath';

function pathsEqual(a: string, b: string): boolean {
  return normalizeGraphResourcePath(a) === normalizeGraphResourcePath(b);
}

/** Update Call Function `subGraphPath` across all loaded graph entity buckets. */
export function cascadeSubGraphPathInLoadedGraphs(from: string, to: string): void {
  const fromNorm = normalizeGraphResourcePath(from);
  const toNorm = normalizeGraphResourcePath(to);
  if (fromNorm === toNorm) return;

  useGraphDataStore.setState((state) => {
    let changed = false;
    const graphEntities = { ...state.graphEntities };

    for (const [graphPath, bucket] of Object.entries(graphEntities)) {
      let bucketChanged = false;
      const nodes = { ...bucket.nodes };

      for (const [nodeId, node] of Object.entries(nodes)) {
        if (
          isCallFunctionNodeType(node.nodeType) &&
          node.subGraphPath &&
          normalizeGraphResourcePath(node.subGraphPath) === fromNorm
        ) {
          nodes[nodeId] = { ...node, subGraphPath: toNorm, graphPath };
          bucketChanged = true;
        }
      }

      if (bucketChanged) {
        graphEntities[graphPath] = { ...bucket, nodes };
        changed = true;
      }
    }

    return changed ? { graphEntities } : state;
  });
}

function remapGraphMetaPath(from: string, to: string): void {
  if (pathsEqual(from, to)) return;

  useGraphMetaStore.setState((state) => {
    const meta = state.graphs[from];
    if (!meta) return state;

    const graphs = { ...state.graphs };
    delete graphs[from];
    graphs[to] = { ...meta, path: to };

    return {
      graphs,
    };
  });
}

function remapVariableScopePaths(from: string, to: string): void {
  const fromNorm = normalizeGraphResourcePath(from);
  const toNorm = normalizeGraphResourcePath(to);
  if (fromNorm === toNorm) return;

  useVariableStore.setState((state) => {
    let changed = false;
    const variables = { ...state.variables };

    for (const [id, variable] of Object.entries(variables)) {
      const scope = variable.scope;
      if (scope.type === 'event' && normalizeGraphResourcePath(scope.eventPath) === fromNorm) {
        variables[id] = { ...variable, scope: { type: 'event', eventPath: toNorm } };
        changed = true;
      } else if (
        scope.type === 'function' &&
        normalizeGraphResourcePath(scope.functionPath) === fromNorm
      ) {
        variables[id] = { ...variable, scope: { type: 'function', functionPath: toNorm } };
        changed = true;
      }
    }

    return changed ? { variables } : state;
  });
}

function remapEditorGraphPaths(from: string, to: string): void {
  if (pathsEqual(from, to)) return;

  const store = useEditorStore.getState();
  const focus = store.detailFocus;

  if (focus?.kind === 'event' || focus?.kind === 'function') {
    if (focus.path === from) {
      store.setDetailFocus({ ...focus, path: to });
    }
  } else if (focus?.kind === 'node' && focus.graphPath === from) {
    store.setDetailFocus({ ...focus, graphPath: to });
  }

  if (store.variablesGraphScopePath === from) {
    store.setVariablesGraphScope(to);
  }
}

/** Migrate only temporary editor UI state for a renamed graph resource. */
export function remapGraphTemporaryUiState(from: string, to: string): void {
  if (pathsEqual(from, to)) return;
  remapEditorGraphPaths(from, to);
  remapGraphViewport(from, to);
  const projectPath = useProjectIOStore.getState().currentPath;
  if (projectPath) {
    remapEditorViewStateGraphPath(projectPath, from, to);
  }
}

/** Cascade all in-memory references when a graph resource path changes on disk. */
export function cascadeGraphPathReferences(from: string, to: string): void {
  if (pathsEqual(from, to)) return;
  cascadeSubGraphPathInLoadedGraphs(from, to);
  remapGraphMetaPath(from, to);
  remapVariableScopePaths(from, to);
  remapEditorGraphPaths(from, to);
  remapGraphViewport(from, to);
  const projectPath = useProjectIOStore.getState().currentPath;
  if (projectPath) {
    remapEditorViewStateGraphPath(projectPath, from, to);
  }
}
