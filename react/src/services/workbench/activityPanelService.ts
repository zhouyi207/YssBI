import { invokeCommand } from "@/services/ipc";
import { parseActivityPanelResponse } from "@/shared/types/dto/activityPanel";
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
  const value = await invoke(previous?.cursor ?? null);
  const snapshot = parseActivityPanelResponse(
    value,
    panelId,
    project?.projectInstanceId ?? null,
    previous,
  );
  if (snapshot) return snapshot;
  // One explicit snapshot recovery for a lost baseline or invalid delta; never publish a partial tree.
  if (previous) {
    const recovered = parseActivityPanelResponse(
      await invoke(null),
      panelId,
      project?.projectInstanceId ?? null,
      null,
    );
    if (
      recovered &&
      (!previous || recovered.document.publicationRevision >= previous.document.publicationRevision)
    )
      return recovered;
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
