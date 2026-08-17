import { logger } from '@/utils/appLogger';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { useResourceStore } from '@/features/core/resource';
import { collectCallFunctionIssuesForBucket } from '@/features/domain/graphDiagnostics';


/** Non-blocking save warning when a graph contains broken Call Function targets. */
export function warnCallFunctionIssuesBeforeSave(graphPath: string): void {
  const bucket = useGraphDataStore.getState().graphEntities[graphPath];
  if (!bucket) return;

  const issues = collectCallFunctionIssuesForBucket(
    graphPath,
    bucket,
    useResourceStore.getState().resources,
  );
  if (issues.length === 0) return;

  logger.graph.warn(
    `Saving graph '${graphPath}' with ${issues.length} broken Call Function reference(s)`,
    'GraphDiagnostics',
  );
}
