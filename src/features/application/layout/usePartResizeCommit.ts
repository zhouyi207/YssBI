import { useEffect } from 'react';
import type { WorkbenchPartId } from '@/features/core/layout/workbenchLayoutDefaults';
import {
  subscribePartResizeCommit,
  type PartResizeCommitDetail,
} from '@/features/core/layout/partResizeNotifier';

/** Invoke callback when a workbench Part size is committed (post-sash, debounced). */
export function usePartResizeCommit(
  partId: WorkbenchPartId,
  onCommit: (pixelSize: number) => void,
): void {
  useEffect(() => {
    return subscribePartResizeCommit((detail: PartResizeCommitDetail) => {
      if (detail.partId === partId) onCommit(detail.pixelSize);
    });
  }, [partId, onCommit]);
}
