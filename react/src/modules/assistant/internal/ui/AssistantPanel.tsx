import { AssistantConversations } from "./AssistantConversations";

export function AssistantPanel() {
  return (
    <div className="h-full min-h-0 w-full min-w-0 overflow-hidden" data-assistant-panel>
      <AssistantConversations />
    </div>
  );
}
