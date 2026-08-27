import {
  useExternalStoreRuntime,
  type AppendMessage,
  type ThreadMessageLike,
} from '@assistant-ui/react';

const EMPTY_ASSISTANT_MESSAGES: readonly ThreadMessageLike[] = Object.freeze([]);

async function ignoreDisabledAssistantSend(_message: AppendMessage): Promise<void> {}

function identityAssistantMessage(message: ThreadMessageLike): ThreadMessageLike {
  return message;
}

export function useAssistantShellRuntime() {
  return useExternalStoreRuntime({
    messages: EMPTY_ASSISTANT_MESSAGES,
    convertMessage: identityAssistantMessage,
    isRunning: false,
    isSendDisabled: true,
    onNew: ignoreDisabledAssistantSend,
  });
}
