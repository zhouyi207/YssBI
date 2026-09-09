import { useMemo } from "react";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import type {
  ActivityItem,
  ActivityPanelDocument,
  ActivityPanelRow,
} from "@/shared/types/domain/activityPanel";
import { useActivityPanelExpansion } from "./useActivityPanelExpansion";

const sections = [
  [
    "event",
    "project.events",
    "sidebar.projectTree.categories.events",
    true,
    "newEvent",
    "canvas.newEventGraph",
    "sidebar.noEvents",
  ],
  [
    "function",
    "project.functions",
    "sidebar.projectTree.categories.functions",
    false,
    "newFunction",
    "canvas.newFunctionGraph",
    "sidebar.noFunctions",
  ],
  [
    "chart",
    "project.charts",
    "sidebar.projectTree.categories.charts",
    true,
    "newChart",
    "contextMenu.sidebar.newChart",
    "chartsSidebar.noCharts",
  ],
  [
    "database",
    "project.data",
    "sidebar.sections.data",
    true,
    "importData",
    "contextMenu.sidebar.importData",
    "sidebar.noData",
  ],
] as const;

/** Project is a synchronous view of the published resource snapshot, not a second IPC query. */
export function useProjectActivityPanelDocument() {
  const resources = useResourceStore((state) => state.resources);
  const publicationRevision = useResourceStore((state) => state.indexRevision);
  const projectInstanceId = useProjectIOStore((state) => state.projectInstanceId);
  const expansion = useActivityPanelExpansion("project");
  const document = useMemo<ActivityPanelDocument>(() => {
    const rows: ActivityPanelRow[] = [];
    const available = projectInstanceId
      ? Object.values(resources).filter((resource) => resource.exists)
      : [];
    for (const [kind, id, title, defaultExpanded, action, actionLabel, empty] of sections) {
      rows.push({
        id,
        depth: 0,
        kind: "category",
        label: { key: title },
        defaultExpanded,
        tools: [{ id: action, label: { key: actionLabel }, icon: "add" }],
        count: null,
      });
      const items = available.filter((resource) => resource.kind === kind);
      for (const resource of items) {
        let item: ActivityItem;
        if (resource.kind === "event" || resource.kind === "function") {
          item = {
            kind: "graph",
            path: resource.id,
            name: resource.name,
            graphType: resource.kind,
          };
        } else if (resource.kind === "chart") {
          item = { kind: "chart", path: resource.id, name: resource.name };
        } else {
          item = {
            kind: "database",
            id: resource.id,
            name: resource.name,
            resourcePath: resource.resourcePath!,
          };
        }
        rows.push({ id: resource.uri, kind: "item", depth: 1, item });
      }
      if (items.length === 0)
        rows.push({
          id: `${id}.empty`,
          kind: "message",
          depth: 1,
          label: { key: empty },
          description: null,
        });
    }
    return {
      schema: "yssbi.activity-panel.v1",
      panelId: "project",
      projectInstanceId,
      publicationRevision,
      title: { key: "activityBar.project" },
      tools: [],
      rows,
      emptyState: null,
    };
  }, [resources, projectInstanceId, publicationRevision]);
  return { document, ...expansion };
}
