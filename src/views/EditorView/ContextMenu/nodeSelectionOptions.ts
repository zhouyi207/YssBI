import type { NodeData } from '@/shared/types';
import type { NodeSelectionOption } from './NodeSelectionPalette';

export interface NodeSelectionGraphSnapshot {
  readonly graphNodes: readonly string[];
  readonly nodes: Readonly<Record<string, Pick<NodeData, 'id' | 'title'>>>;
}

const EMPTY_NODE_SELECTION_OPTIONS: readonly NodeSelectionOption[] = [];
const optionsByGraph = new WeakMap<
  NodeSelectionGraphSnapshot,
  readonly NodeSelectionOption[]
>();

export function getNodeSelectionOptions(
  graph?: NodeSelectionGraphSnapshot,
): readonly NodeSelectionOption[] {
  if (!graph) return EMPTY_NODE_SELECTION_OPTIONS;

  const cached = optionsByGraph.get(graph);
  if (cached) return cached;

  const options = graph.graphNodes.flatMap((nodeId) => {
    const node = graph.nodes[nodeId];
    return node ? [{ id: node.id, title: node.title }] : [];
  });
  optionsByGraph.set(graph, options);
  return options;
}
