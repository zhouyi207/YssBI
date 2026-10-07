import { produce } from "immer";
import { shallow } from "zustand/shallow";
import { nodeFileEntries } from "@/shared/types/domain/project";
import type { ChartDocument } from "@/shared/types/domain/chart";
import {
  prepareResourceProjectionSnapshot,
  resourceKey,
  type DocumentState,
  type ProjectResourceMeta,
  type ResourceKey,
  type FileSnapshots,
} from "@/features/core/resource";
import { buildProjectResourceState } from "@/features/application/project/authoritativeProjectLoadPlan";
import type { ProjectSnapshotPreparation } from "./projectPublicationCoordinator";
import { hasPendingDocumentInput } from "@/features/application/resource/documentInputs";

interface SnapshotResources {
  documents: Record<ResourceKey, DocumentState>;
  resources: Record<ResourceKey, ProjectResourceMeta>;
  chartDocuments: Record<string, ChartDocument>;
  fileSnapshots: FileSnapshots;
}

function remapDocuments(
  documents: Record<ResourceKey, DocumentState>,
  plan: ProjectSnapshotPreparation,
): void {
  const graphKind = new Map(nodeFileEntries(plan.index).map((graph) => [graph.path, graph.type]));
  for (const [from, to] of plan.pathRemaps) {
    const kind = graphKind.get(to);
    if (!kind) continue;
    const fromKey = resourceKey({ id: from, kind });
    const toKey = resourceKey({ id: to, kind });
    const source = documents[fromKey];
    if (!source) continue;
    documents[toKey] = { ...source, resourceKey: toKey };
    delete documents[fromKey];
  }
  for (const [from, to] of plan.filePathRemaps) {
    const kind =
      [...plan.index.minds, ...plan.index.docs].find((document) => document.path === to)?.kind ??
      "chart";
    const fromKey = resourceKey({ id: from, kind });
    const toKey = resourceKey({ id: to, kind });
    const source = documents[fromKey];
    if (!source) continue;
    documents[toKey] = { ...source, resourceKey: toKey };
    delete documents[fromKey];
  }
}

function remapResources(
  resources: Record<ResourceKey, ProjectResourceMeta>,
  plan: ProjectSnapshotPreparation,
): void {
  const graphByPath = new Map(nodeFileEntries(plan.index).map((graph) => [graph.path, graph]));
  for (const [from, to] of plan.pathRemaps) {
    const graph = graphByPath.get(to);
    if (!graph) continue;
    const fromKey = resourceKey({ id: from, kind: graph.type });
    const toKey = resourceKey({ id: to, kind: graph.type });
    const source = resources[fromKey];
    if (!source) continue;
    resources[toKey] = { ...source, id: to, uri: toKey, name: graph.name, kind: graph.type };
    delete resources[fromKey];
  }
  const fileByPath = new Map<
    string,
    { name: string; revision: number; kind: "chart" | "mind" | "doc" }
  >([
    ...plan.index.charts.map(
      (chart) =>
        [
          chart.chartPath,
          { name: chart.name, revision: chart.revision, kind: "chart" as const },
        ] as const,
    ),
    ...[...plan.index.minds, ...plan.index.docs].map(
      (document) => [document.path, document] as const,
    ),
  ]);
  for (const [from, to] of plan.filePathRemaps) {
    const file = fileByPath.get(to);
    if (!file) continue;
    const kind = file.kind;
    const fromKey = resourceKey({ id: from, kind });
    const toKey = resourceKey({ id: to, kind });
    const source = resources[fromKey];
    if (!source) continue;
    resources[toKey] = {
      ...source,
      id: to,
      uri: toKey,
      name: file.name,
      revision: file.revision,
      kind,
    };
    delete resources[fromKey];
  }
}

function applyDocumentPatches(
  documents: Record<ResourceKey, DocumentState>,
  patches: ReturnType<typeof prepareResourceProjectionSnapshot>["documentPatches"],
): void {
  for (const { key, patch } of patches) {
    const previous = documents[key];
    if (previous) Object.assign(previous, patch);
    else
      documents[key] = {
        resourceKey: key,
        loaded: true,
        dirty: patch.conflict ?? false,
        stale: patch.stale ?? false,
        missing: patch.missing ?? false,
        conflict: patch.conflict ?? false,
      };
  }
}

export function prepareSnapshotResources(
  plan: ProjectSnapshotPreparation,
  current: SnapshotResources,
  refreshedKeys: readonly ResourceKey[],
): SnapshotResources {
  return produce(current, (draft) => {
    const { documents, resources, chartDocuments, fileSnapshots } = draft;
    remapDocuments(documents, plan);
    for (const key of plan.deletedResources) delete documents[key];
    const authoritativeChartPaths = new Set(plan.index.charts.map((chart) => chart.chartPath));
    for (const [from, to] of plan.filePathRemaps) {
      const source = chartDocuments[from];
      if (!source) continue;
      chartDocuments[to] = source;
      delete chartDocuments[from];
    }
    for (const path of Object.keys(chartDocuments)) {
      if (
        !authoritativeChartPaths.has(path) &&
        !documents[resourceKey({ id: path, kind: "chart" })]?.dirty
      )
        delete chartDocuments[path];
    }
    for (const [path, document] of plan.chartDocuments) {
      if (!documents[resourceKey({ id: path, kind: "chart" })]?.dirty)
        chartDocuments[path] = document;
    }

    remapResources(resources, plan);
    for (const key of plan.deletedResources) delete resources[key];
    const incoming = Object.values(
      buildProjectResourceState({
        eventGraphs: plan.index.eventGraphs,
        functionGraphs: plan.index.functionGraphs,
        charts: plan.index.charts,
        minds: plan.index.minds,
        docs: plan.index.docs,
        databases: plan.index.databases,
        loadedChartPaths: new Set(Object.keys(chartDocuments)),
      }).resources,
    );
    const { resources: projectedResources, documentPatches } = prepareResourceProjectionSnapshot(
      incoming,
      resources,
      documents,
    );
    const incomingKeys = new Set(projectedResources.map(resourceKey));
    for (const key of Object.keys(resources) as ResourceKey[]) {
      if (!incomingKeys.has(key)) delete resources[key];
    }
    for (const resource of projectedResources) {
      const key = resourceKey(resource);
      if (!shallow(resources[key], resource)) resources[key] = resource;
    }
    applyDocumentPatches(documents, documentPatches);
    for (const [kind, entries] of [
      ["mind", plan.index.minds],
      ["doc", plan.index.docs],
    ] as const) {
      const paths = new Set(entries.map((file) => file.path));
      for (const path of Object.keys(fileSnapshots[kind])) {
        const key = resourceKey({ kind, id: path });
        if (
          !paths.has(path) &&
          (plan.deletedResources.has(key) ||
            plan.filePathRemaps.has(path) ||
            (!documents[key]?.dirty &&
              !resources[key]?.hasDirtyDocument &&
              !hasPendingDocumentInput(path)))
        ) {
          delete fileSnapshots[kind][path];
          delete documents[key];
        }
      }
    }
    for (const key of refreshedKeys) {
      if (documents[key]?.dirty) continue;
      if (documents[key])
        Object.assign(documents[key], { stale: false, missing: false, conflict: false });
      if (resources[key])
        Object.assign(resources[key], {
          loaded: true,
          hasStaleDocument: false,
          hasConflictDocument: false,
        });
    }
  });
}
