import {
  AssistantRuntimeProvider as AssistantUiRuntimeProvider,
} from '@assistant-ui/react';
import type { PropsWithChildren } from 'react';

import { useAssistantShellRuntime } from './assistantShellRuntime';

export function AssistantRuntimeProvider({ children }: PropsWithChildren) {
  const runtime = useAssistantShellRuntime();

  return (
    <AssistantUiRuntimeProvider runtime={runtime}>
      {children}
    </AssistantUiRuntimeProvider>
  );
}
