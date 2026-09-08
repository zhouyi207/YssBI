import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { pluginService } from "@/services/plugins/pluginService";
import type { InstalledPlugin, PluginView, ViewSession } from "@/shared/types/plugins/generated";
import { useApplicationSettings } from "@/features/application/settings/applicationSettings";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import { fulfillPluginUiRequest } from "@/features/application/plugins/pluginActions";
import { PluginViewSession, pluginViewFailure, type PluginViewFailure } from "./pluginViewSession";

interface ViewBinding {
  readonly session: ViewSession;
  valid: boolean;
  loaded: boolean;
  channel: MessageChannel | null;
}

function revoke(binding: ViewBinding | null) {
  if (!binding) return;
  binding.valid = false;
  binding.channel?.port1.close();
  binding.channel?.port2.close();
  binding.channel = null;
}

export function usePluginView({
  plugin,
  viewId,
  onOpen,
  visible = true,
}: {
  plugin: InstalledPlugin | undefined;
  viewId: string;
  onOpen(pluginId: string, view: PluginView): void;
  visible?: boolean;
}) {
  const { i18n } = useTranslation();
  const { theme } = useApplicationSettings();
  const currentProjectId = useProjectIOStore((state) => state.projectInstanceId);
  const projectId =
    plugin?.manifest.contributes.views.find((view) => view.id === viewId)?.scope === "project"
      ? currentProjectId
      : undefined;
  const [session, setSession] = useState<ViewSession | null>(null);
  const [error, setError] = useState<PluginViewFailure | null>(null);
  const [retry, setRetry] = useState(0);
  const frame = useRef<HTMLIFrameElement>(null);
  const binding = useRef<ViewBinding | null>(null);
  const lease = useRef<PluginViewSession | null>(null);
  if (!lease.current) lease.current = new PluginViewSession();
  const pluginId = plugin?.manifest.id;
  const generation = plugin?.installationGeneration;
  const enabled = plugin?.enabled;
  const context = () => {
    const style = getComputedStyle(document.documentElement);
    const names = [
      "background",
      "foreground",
      "card",
      "card-foreground",
      "popover",
      "popover-foreground",
      "primary",
      "primary-foreground",
      "secondary",
      "secondary-foreground",
      "muted",
      "muted-foreground",
      "accent",
      "accent-foreground",
      "destructive",
      "border",
      "input",
      "ring",
      "chart-1",
      "chart-2",
      "chart-3",
      "chart-4",
      "chart-5",
    ];
    return {
      language: i18n.language,
      viewId,
      visible,
      themeMode: theme.mode,
      theme: Object.fromEntries(
        names.map((name) => [`--${name}`, style.getPropertyValue(`--${name}`)]),
      ),
    };
  };

  useEffect(() => {
    setSession(null);
    setError(null);
    if (!pluginId || !enabled) return;
    const owner = lease.current!;
    let disposed = false;
    void owner
      .open(pluginId, viewId)
      .then((value) => {
        if (disposed || !value) return;
        binding.current = { session: value, valid: true, loaded: false, channel: null };
        setSession(value);
      })
      .catch((failure) => {
        if (!disposed) setError(pluginViewFailure(failure, "attach"));
      });
    return () => {
      disposed = true;
      revoke(binding.current);
      binding.current = null;
      // The owner retains a failed release and retries it before any successor attach.
      void owner.close().catch(() => undefined);
    };
  }, [pluginId, generation, enabled, viewId, projectId, retry]);

  useEffect(() => {
    binding.current?.channel?.port1.postMessage({ type: "context", ...context() });
  }, [theme, i18n.language, visible]);

  const onLoad = () => {
    const active = binding.current;
    if (!active || !active.valid || active.session !== session) return;
    const isCurrent = () => binding.current === active && active.valid;
    const fail = (failure: PluginViewFailure) => {
      if (!isCurrent()) return;
      revoke(active);
      setError(failure);
      void lease.current!.close().catch((error) => {
        if (binding.current === active) setError(pluginViewFailure(error, "detach"));
      });
    };
    if (active.loaded) {
      // Display changes keep the document attached. A second load is a new,
      // unauthenticated document: revoke it and require an explicit fresh attach.
      fail({ phase: "navigation", code: "plugin_view_navigation", incidentId: null });
      return;
    }
    active.loaded = true;
    const channel = new MessageChannel();
    active.channel = channel;
    const pending = new Set<string>();
    channel.port1.onmessage = async (event) => {
      if (!isCurrent()) return;
      const message = event.data;
      if (
        !message ||
        typeof message.id !== "string" ||
        !message.id ||
        message.id.length > 128 ||
        typeof message.method !== "string" ||
        message.method.length > 128 ||
        pending.size >= 16 ||
        pending.has(message.id)
      )
        return;
      try {
        if (new TextEncoder().encode(JSON.stringify(message)).byteLength > 2 * 1024 * 1024) return;
      } catch {
        return;
      }
      pending.add(message.id);
      try {
        const response = await pluginService.call(
          active.session.sessionId,
          message.method,
          message.input ?? null,
        );
        if (!isCurrent()) return;
        const result = await fulfillPluginUiRequest(active.session.sessionId, response);
        if (!isCurrent()) return;
        if (result && typeof result === "object" && "openView" in result) {
          const open = (result as { openView: { pluginId: string; view: PluginView } }).openView;
          if (open.pluginId === pluginId) onOpen(open.pluginId, open.view);
        }
        channel.port1.postMessage({ id: message.id, result });
      } catch (error) {
        if (!isCurrent()) return;
        const failure = pluginViewFailure(error, "bridge");
        channel.port1.postMessage({
          id: message.id,
          error: { code: failure.code, details: null, incidentId: failure.incidentId },
        });
        if (
          ["plugin_stale_context", "plugin_process_exited", "plugin_request_timeout"].includes(
            failure.code,
          )
        )
          fail(failure);
      } finally {
        pending.delete(message.id);
      }
    };
    try {
      frame.current?.contentWindow?.postMessage({ type: "yssbi:plugin-init", ...context() }, "*", [
        channel.port2,
      ]);
    } catch (error) {
      fail(pluginViewFailure(error, "bridge"));
    }
  };

  return { frame, session, error, onLoad, retry: () => setRetry((value) => value + 1) };
}
