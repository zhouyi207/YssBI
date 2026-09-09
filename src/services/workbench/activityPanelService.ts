import { invokeCommand } from "@/services/ipc";
import { parseActivityPanelUpdate } from "@/shared/types/dto/activityPanel";
import type { ActivityPanelId, ActivityPanelSnapshot } from "@/shared/types/domain/activityPanel";
import { IpcError } from "@/services/ipc/ipcError";

export async function getActivityPanelDocument(
  panelId: ActivityPanelId,
  project: { projectInstanceId: string } | null,
  locale: string,
  previous: ActivityPanelSnapshot | null = null,
): Promise<ActivityPanelSnapshot> {
  const invoke = (cursor: string | null) =>
    invokeCommand<unknown>("get_activity_panel_document", {
      panelId,
      projectInstanceId: project?.projectInstanceId ?? null,
      locale,
      cursor,
    });
  const matchesScope = (snapshot: ActivityPanelSnapshot) =>
    snapshot.document.panelId === panelId &&
    snapshot.document.projectInstanceId === (project?.projectInstanceId ?? null) &&
    (!previous || snapshot.document.publicationRevision >= previous.document.publicationRevision);
  const value = await invoke(previous?.cursor ?? null);
  const snapshot = parseActivityPanelUpdate(value, previous);
  if (snapshot && matchesScope(snapshot)) return snapshot;
  // One explicit snapshot recovery for a lost baseline or invalid delta; never publish a partial tree.
  if (previous) {
    const recovered = parseActivityPanelUpdate(await invoke(null), null);
    if (recovered && matchesScope(recovered)) return recovered;
  }
  throw new IpcError({
    kind: "malformed",
    command: "get_activity_panel_document",
    code: "activity_panel_contract_invalid",
    details: null,
    incidentId: null,
    cause: null,
  });
}
