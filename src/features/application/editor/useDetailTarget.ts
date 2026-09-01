import { useMemo } from "react";

import { useLogStore } from "@/features/application/log";
import { useEditorStore } from "@/features/core/editor/stores/useEditorStore";
import { resolveDetailTarget } from "./resolveDetailTarget";
import type { DetailTarget } from "@/shared/types/ui/detail";

/** Application composition of editor focus and the selected diagnostic log. */
export function useDetailTarget(): DetailTarget | null {
  const detailFocus = useEditorStore((state) => state.detailFocus);
  const selectedLog = useLogStore((state) => state.selectedLog);

  return useMemo(
    () => resolveDetailTarget({ detailFocus, selectedLog }),
    [detailFocus, selectedLog],
  );
}
