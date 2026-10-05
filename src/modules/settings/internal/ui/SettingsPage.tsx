import type { ComponentProps, ReactNode } from "react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { SettingsPageHeader } from "./SettingsPageHeader";

export function SettingsPage({
  breadcrumbs,
  actions,
  notice,
  children,
}: ComponentProps<typeof SettingsPageHeader> & {
  notice?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="settings-page">
      <SettingsPageHeader breadcrumbs={breadcrumbs} actions={actions} />
      {notice && <div className="settings-page-notices">{notice}</div>}
      <ScrollArea className="min-h-0 flex-1" orientation="vertical">
        <div className="settings-content">{children}</div>
      </ScrollArea>
    </div>
  );
}
