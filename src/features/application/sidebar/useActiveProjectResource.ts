import { useSyncExternalStore } from "react";
import { useShallow } from "zustand/react/shallow";
import { workbenchLayoutRead } from "@/modules/workbench/public";
import { resourceKey, type ProjectResourceMeta } from "@/features/core/resource";
import { useResourceRead } from "@/features/core/resource/read";

export function useActiveProjectResource(): Pick<ProjectResourceMeta, "id" | "kind"> | null {
  useSyncExternalStore(
    workbenchLayoutRead.subscribeActivePanel,
    workbenchLayoutRead.getActiveSnapshot,
    workbenchLayoutRead.getActiveSnapshot,
  );
  const active = workbenchLayoutRead.getActiveEditorPanel();
  return useResourceRead(
    useShallow((snapshot) => {
      const resource = active
        ? snapshot.resources[
            resourceKey({ id: active.metadata.resourceRef, kind: active.metadata.resourceKind })
          ]
        : null;
      return resource ? { id: resource.id, kind: resource.kind } : null;
    }),
  );
}
