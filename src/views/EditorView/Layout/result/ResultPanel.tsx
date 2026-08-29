import type { IDockviewPanelProps } from 'dockview-react';

import type { WorkbenchPanelParams } from '@/features/core/dockview';
import { ResultContent } from './ResultContent';

export function ResultPanel(
  props: IDockviewPanelProps<WorkbenchPanelParams>,
) {
  const metadata = props.params.metadata;
  if (metadata.role !== 'result') return null;
  return (
    <div
      className="flex h-full min-h-0 flex-col overflow-hidden bg-background"
      data-workbench-result-panel
    >
      <ResultContent
        key={metadata.resultId}
        resultId={metadata.resultId}
      />
    </div>
  );
}
