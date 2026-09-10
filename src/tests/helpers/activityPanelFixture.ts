import type { ProjectIndexRow, ProjectIndexSnapshot } from "@/shared/types/domain/project";
import { PROJECT_ACTIVITY_PANEL_IDS } from "@/shared/types/domain/activityPanel";
import type {
  ActivityPanelDocument,
  ActivityPanelId,
  ActivityPanelRow,
  ActivityTool,
} from "@/shared/types/domain/activityPanel";

export function activityPanelFixture(
  panelId: ActivityPanelId,
  rows: ActivityPanelRow[],
  tools: ActivityTool[] = [],
): ActivityPanelDocument {
  return {
    schema: "yssbi.activity-panel.v1",
    panelId,
    projectInstanceId: panelId === "project" || panelId === "nodes" ? "project-1" : null,
    publicationRevision: 1,
    title: { key: `activityBar.${panelId}` },
    tools,
    rows,
    emptyState:
      panelId === "commands"
        ? {
            title: { key: "sidebar.noActiveGraph" },
            description: { key: "sidebar.noActiveGraphDescription" },
          }
        : null,
  };
}
export function categoryFixture(
  id: string,
  title: string,
  depth = 0,
  defaultExpanded = false,
): ActivityPanelRow & { kind: "category" } {
  return {
    kind: "category",
    id,
    depth,
    label: { text: title },
    defaultExpanded,
    tools: [],
    count: null,
  };
}

export function projectIndexSnapshotFixture(index: ProjectIndexRow): ProjectIndexSnapshot {
  return {
    index,
    activityPanels: Object.fromEntries(
      PROJECT_ACTIVITY_PANEL_IDS.map((panelId) => [
        panelId,
        {
          cursor: `${panelId}-${index.publicationRevision}`,
          document: {
            ...activityPanelFixture(panelId, []),
            projectInstanceId: index.projectInstanceId,
            publicationRevision: index.publicationRevision,
          },
        },
      ]),
    ) as ProjectIndexSnapshot["activityPanels"],
  };
}
