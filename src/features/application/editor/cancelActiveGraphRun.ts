import { useExecutionStore } from "@/features/core/execution";
import { ProjectService } from "@/services/project/projectService";

type CancelActiveGraphRunDependencies = {
  cancelGraphRun: (runId: string) => Promise<boolean>;
};

const productionDependencies: CancelActiveGraphRunDependencies = {
  cancelGraphRun: (runId) => ProjectService.cancelGraphRun(runId),
};

export async function cancelActiveGraphRun(
  graphPath: string,
  dependencies: CancelActiveGraphRunDependencies = productionDependencies,
): Promise<boolean> {
  const runId = useExecutionStore.getState().getGraph(graphPath).runId;
  if (!runId) return false;
  return dependencies.cancelGraphRun(runId);
}
