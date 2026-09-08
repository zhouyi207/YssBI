import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import type { InstalledPlugin, PluginView } from "@/shared/types/plugins/generated";
import { usePluginView } from "@/features/application/plugins/usePluginView";
import { pluginViewFailureMessage } from "@/features/application/plugins/pluginViewSession";

export function PluginViewFrame(props: {
  plugin: InstalledPlugin | undefined;
  viewId: string;
  visible?: boolean;
  onOpen(pluginId: string, view: PluginView): void;
}) {
  const { t } = useTranslation();
  const { frame, session, error, onLoad, retry } = usePluginView(props);
  if (!props.plugin || !props.plugin.enabled)
    return (
      <div className="space-y-3 p-4 text-xs text-muted-foreground">
        <p>{t(props.plugin ? "plugins.disabledView" : "plugins.missingView")}</p>
      </div>
    );
  if (error)
    return (
      <div role="alert" className="space-y-3 p-4 text-xs">
        <p>{t("plugins.viewFailed")}</p>
        <p className="text-muted-foreground">{t(pluginViewFailureMessage(error))}</p>
        <code className="block text-muted-foreground">{error.code}</code>
        {error.incidentId && (
          <code className="block text-muted-foreground">{error.incidentId}</code>
        )}
        <Button variant="outline" onClick={retry}>
          {t("plugins.recheck")}
        </Button>
      </div>
    );
  if (!session)
    return (
      <div role="status" className="p-4 text-xs text-muted-foreground">
        {t("common.loading")}
      </div>
    );
  return (
    <iframe
      key={session.sessionId}
      ref={frame}
      title={
        props.plugin.manifest.contributes.views.find((view) => view.id === props.viewId)?.title ??
        props.plugin.manifest.name
      }
      sandbox="allow-scripts"
      referrerPolicy="no-referrer"
      className="h-full min-h-0 w-full border-0 bg-background"
      srcDoc={session.html}
      onLoad={onLoad}
    />
  );
}
