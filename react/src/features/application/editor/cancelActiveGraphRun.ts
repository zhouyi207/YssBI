import { useExecutionStore } from "@/features/core/execution";
import { ProjectService } from "@/services/project/projectService";

type CancelActiveGraphRunDependencies = {
  cancelGraphRun: (executionSessionId: string, runId: string) => Promise<boolean>;
};

const productionDependencies: CancelActiveGraphRunDependencies = {
  cancelGraphRun: (executionSessionId, runId) =>
    ProjectService.cancelGraphRun(executionSessionId, runId),
};

export async function cancelActiveGraphRun(
  graphPath: string,
  dependencies: CancelActiveGraphRunDependencies = productionDependencies,
): Promise<boolean> {
  const run = useExecutionStore.getState().getGraph(graphPath).run;
  if (!run) return false;
  return dependencies.cancelGraphRun(run.executionSessionId, run.runId);
}
