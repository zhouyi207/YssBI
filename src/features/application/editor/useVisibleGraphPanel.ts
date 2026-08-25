import { useEffect, useState } from 'react';
import type { IDockviewPanelProps } from 'dockview-react';

import type { WorkbenchPanelParams } from '@/features/core/dockview';
import {
  synchronizeVisibleGraphPanel,
  type VisibleGraphPanelScope,
} from './synchronizeVisibleGraphPanel';

type VisiblePanelApi = Pick<
  IDockviewPanelProps<WorkbenchPanelParams>['api'],
  'isVisible' | 'onDidVisibilityChange' | 'onDidGroupChange'
>;

/** Synchronize only while Dockview keeps this panel in the visible layout. */
export function useVisibleGraphPanel(
  api: VisiblePanelApi,
  scope: VisibleGraphPanelScope,
): void {
  const [isVisible, setIsVisible] = useState(() => api.isVisible);

  useEffect(() => {
    const updateVisibility = () => setIsVisible(api.isVisible);
    const visibilityDisposable = api.onDidVisibilityChange(updateVisibility);
    const groupDisposable = api.onDidGroupChange(updateVisibility);
    updateVisibility();
    return () => {
      visibilityDisposable.dispose();
      groupDisposable.dispose();
    };
  }, [api]);

  useEffect(() => {
    if (!isVisible) return;
    void synchronizeVisibleGraphPanel({
      groupId: scope.groupId,
      graphPath: scope.graphPath,
    }).catch(() => undefined);
  }, [isVisible, scope.groupId, scope.graphPath]);
}
