import { useEffect, useState } from "react";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import { loadChartDocumentForView } from "./chartViewActions";

/** Views share the read; each mounted view only owns its error/retry presentation. */
export function useChartDocumentLoad(chartPath: string, hasDocument: boolean) {
  const [failed, setFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const projectInstanceId = useProjectIOStore((state) => state.projectInstanceId);
  useEffect(() => {
    if (hasDocument) return;
    let current = true;
    setFailed(false);
    void loadChartDocumentForView(chartPath).then((result) => {
      if (current && !result) setFailed(true);
    });
    return () => {
      current = false;
    };
  }, [chartPath, hasDocument, attempt, projectInstanceId]);
  return { failed, retry: () => setAttempt((value) => value + 1) };
}
