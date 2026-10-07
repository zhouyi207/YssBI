import { shallow } from "zustand/shallow";
import type { ChartDocument } from "@/shared/types/domain/chart";
import type { DocSnapshot } from "@/shared/types/domain/doc";
import type { MindSnapshot } from "@/shared/types/domain/mind";

export function sameChartDocument(left: ChartDocument, right: ChartDocument): boolean {
  return (
    left.schemaVersion === right.schemaVersion &&
    left.databaseId === right.databaseId &&
    left.chartType === right.chartType &&
    left.encodings.x === right.encodings.x &&
    left.encodings.y === right.encodings.y
  );
}

export function shareChartDocument(
  previous: ChartDocument | undefined,
  next: ChartDocument,
): ChartDocument {
  if (previous && sameChartDocument(previous, next)) return previous;
  return previous && shallow(previous.encodings, next.encodings)
    ? { ...next, encodings: previous.encodings }
    : next;
}

export function shareDocSnapshot(
  previous: DocSnapshot | undefined,
  next: DocSnapshot,
): DocSnapshot {
  if (!previous || previous === next) return next;
  const shared = {
    ...next,
    version: shallow(previous.version, next.version) ? previous.version : next.version,
  };
  return shallow(previous, shared) ? previous : shared;
}

export function shareMindSnapshot(
  previous: MindSnapshot | undefined,
  next: MindSnapshot,
): MindSnapshot {
  if (!previous || previous === next) return next;
  let content = next.content;
  if (previous.content !== content) {
    const nodesById = new Map(previous.content.nodes.map((node) => [node.id, node]));
    const nodes = content.nodes.map((node) => {
      const before = nodesById.get(node.id);
      const shared =
        before?.reference && node.reference && shallow(before.reference, node.reference)
          ? { ...node, reference: before.reference }
          : node;
      return before && shallow(before, shared) ? before : shared;
    });
    content = {
      ...content,
      nodes: shallow(previous.content.nodes, nodes) ? previous.content.nodes : nodes,
    };
    if (shallow(previous.content, content)) content = previous.content;
  }
  const shared = {
    ...next,
    content,
    version: shallow(previous.version, next.version) ? previous.version : next.version,
  };
  return shallow(previous, shared) ? previous : shared;
}
