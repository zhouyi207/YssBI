import { useEffect, useMemo, useState } from "react";
import { useResultSession } from "@/features/application/window/useResultSession";
import { ResultService } from "@/services/result/resultService";
import { resultReferenceKey } from "@/shared/types/domain/result";
import {
  useCurrentWindowActions,
  type CurrentWindowActions,
} from "@/features/application/window/useCurrentWindowActions";
import { loadPresentationWindow, type PresentationWindowState } from "./loadPresentationWindow";
import { parsePresentationWindowQuery } from "./parsePresentationWindowQuery";

export function usePresentationWindow() {
  const query = useMemo(() => parsePresentationWindowQuery(), []);
  const reference = query.reference;
  const [state, setState] = useState<PresentationWindowState>(() =>
    reference && query.leaseId ? { status: "loading" } : { status: "missing_result_id" },
  );

  const sessionActive = useResultSession(reference);
  const windowActions = useCurrentWindowActions();

  useEffect(() => {
    let cancelled = false;

    const revealWindow = async (title?: string) => {
      if (title) await windowActions.setTitle(title);
      await windowActions.show();
    };

    if (!sessionActive) {
      setState({ status: "not_found" });
      void windowActions.close();
      return;
    }

    if (!reference || !query.leaseId) {
      void revealWindow();
      return;
    }

    void (async () => {
      let next: PresentationWindowState;
      try {
        const held = await ResultService.claim(query.leaseId!);
        if (resultReferenceKey(held.descriptor) !== resultReferenceKey(reference))
          throw new Error("Mismatched result lease");
        next = await loadPresentationWindow(reference);
      } catch {
        next = { status: "not_found" };
      }
      if (cancelled) return;
      setState(next);
      if (next.status === "ready") {
        await revealWindow(next.descriptor.title);
        return;
      }
      await revealWindow();
    })();

    return () => {
      cancelled = true;
    };
  }, [reference, query.leaseId, windowActions, sessionActive]);

  return {
    reference,
    state,
    windowActions: windowActions as CurrentWindowActions,
  };
}
