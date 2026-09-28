import { useSyncExternalStore } from "react";
import { workbenchLayoutRead } from "@/modules/workbench/public";
import { resourceKey, type ProjectResourceMeta } from "@/features/core/resource";
import { useResourceRead } from "@/features/core/resource/read";

export function useActiveProjectResource(): ProjectResourceMeta | null {
  useSyncExternalStore(
    workbenchLayoutRead.subscribeActivePanel,
    workbenchLayoutRead.getActiveSnapshot,
    workbenchLayoutRead.getActiveSnapshot,
  );
  const active = workbenchLayoutRead.getActiveEditorPanel();
  return useResourceRead((snapshot) =>
    active
      ? (snapshot.resources[
          resourceKey({ id: active.metadata.resourceRef, kind: active.metadata.resourceKind })
        ] ?? null)
      : null,
  );
}
