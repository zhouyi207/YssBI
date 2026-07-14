import { LogPanelProvider } from '@/views/LogView/logPanelContext';
import { LogPanel } from '@/views/LogView/LogPanel';

export function PanelPart() {
  return (
    <LogPanelProvider variant="embedded">
      <LogPanel />
    </LogPanelProvider>
  );
}
