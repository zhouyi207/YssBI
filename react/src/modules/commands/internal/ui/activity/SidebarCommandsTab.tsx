import { useTranslation } from "react-i18next";
import { useEditorHistoryAvailability } from "@/features/application/editor";
import { useActivityPanelDocument } from "@/features/application/sidebar/useActivityPanelDocument";
import { ActivityPanelDocumentView } from "@/modules/workbench/public";

export function SidebarCommandsTab() {
  const { t } = useTranslation();
  const query = useActivityPanelDocument("commands");
  const { activeResourceRef, canUndo, canRedo, pending } = useEditorHistoryAvailability();
  return (
    <ActivityPanelDocumentView
      panelId="commands"
      document={query.document}
      error={query.error}
      expanded={query.expanded}
      onExpandedChange={query.setExpanded}
      onRetry={query.refresh}
      empty={!activeResourceRef}
      renderItem={(item) => {
        if (item.kind !== "command") return null;
        const available = item.id === "undo" ? canUndo : canRedo;
        return (
          <div className="flex h-7 items-center justify-between px-4 text-xs text-muted-foreground">
            <span>{"key" in item.label ? t(item.label.key) : item.label.text}</span>
            <span aria-label={available ? "available" : "unavailable"}>
              {pending ? "…" : available ? "✓" : "—"}
            </span>
          </div>
        );
      }}
    />
  );
}
