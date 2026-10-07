import { workbenchLayoutControl } from "@/modules/workbench/public";
import { workbenchLayoutRead } from "@/modules/workbench/public";

export async function resolveEditorOpenTargetGroupId(
  explicitGroupId?: string | null,
): Promise<string> {
  const groupIds = new Set(
    workbenchLayoutRead
      .listGroups()
      .filter((group) => group.location.type === "grid")
      .map((group) => group.groupId),
  );
  if (explicitGroupId && groupIds.has(explicitGroupId)) return explicitGroupId;

  return workbenchLayoutControl.ensureCentralGroup();
}
