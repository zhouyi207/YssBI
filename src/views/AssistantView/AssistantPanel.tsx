import { AssistantRuntimeProvider } from '@/features/application/assistant/AssistantRuntimeProvider';

import { AssistantThread } from './AssistantThread';

export function AssistantPanel() {
  return (
    <div className="h-full min-h-0 w-full min-w-0 overflow-hidden" data-assistant-panel>
      <AssistantRuntimeProvider>
        <AssistantThread />
      </AssistantRuntimeProvider>
    </div>
  );
}
