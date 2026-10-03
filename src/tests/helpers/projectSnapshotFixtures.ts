import { buildFileResourceMeta, resourceKey } from "@/features/core/resource";
import type { ProjectSnapshotPreparation } from "@/features/application/editorMutation/projectPublicationCoordinator";
import type { prepareSnapshotResources } from "@/features/application/editorMutation/projectSnapshotResources";

export function projectSnapshotFixture(count: number) {
  const current: Parameters<typeof prepareSnapshotResources>[1] = {
    resources: {},
    documents: {},
    chartDocuments: {},
    fileSnapshots: { mind: {}, doc: {} },
  };
  const plan: ProjectSnapshotPreparation = {
    projectInstanceId: "project-1",
    epoch: 1,
    publicationRevision: 1,
    index: {
      projectInstanceId: "project-1",
      projectName: "Project",
      exportTime: "",
      publicationRevision: 1,
      eventGraphs: [],
      functionGraphs: [],
      databases: [],
      charts: [],
      minds: [],
      docs: [],
    },
    activityPanels: [],
    graphSessions: new Map(),
    chartDocuments: new Map(),
    pathRemaps: new Map(),
    filePathRemaps: new Map(),
    deletedResources: new Set(),
  };
  for (let index = 0; index < count; index++) {
    const path = `charts/Chart-${index}.yssbi-chart`;
    const key = resourceKey({ id: path, kind: "chart" });
    plan.index.charts.push({
      chartPath: path,
      name: `Chart ${index}`,
      revision: 1,
      databaseId: "data",
      chartType: "scatter",
    });
    current.resources[key] = buildFileResourceMeta("chart", path, `Chart ${index}`, {
      loaded: true,
      revision: 1,
    });
    current.documents[key] = {
      resourceKey: key,
      loaded: true,
      dirty: false,
      stale: false,
      missing: false,
      conflict: false,
    };
    current.chartDocuments[path] = {
      schemaVersion: 1,
      databaseId: "data",
      chartType: "scatter",
      encodings: { x: "x", y: "y" },
    };
  }
  return { current, plan };
}
