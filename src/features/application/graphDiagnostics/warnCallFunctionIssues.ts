import { logger } from "@/utils/appLogger";
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { useResourceStore } from '@/features/core/resource';
import { collectCallFunctionIssuesForBucket } from '@/features/domain/graphDiagnostics';
import { i18n } from '@/app/i18n';

/** Non-blocking save warning when a graph contains broken Call Function targets. */
export function warnCallFunctionIssuesBeforeSave(graphPath: string): boolean {
  const bucket = useGraphDataStore.getState().graphEntities[graphPath];
  if (!bucket) return true;

  const issues = collectCallFunctionIssuesForBucket(
    graphPath,
    bucket,
    useResourceStore.getState().resources,
  );
  if (issues.length === 0) return true;

  logger.notify.warn(i18n.t('graphDiagnostics.callFunctionSaveWarning', { count: issues.length }), "UI");
  return true;
}
