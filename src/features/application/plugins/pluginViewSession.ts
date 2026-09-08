import { pluginService } from "@/services/plugins/pluginService";
import { normalizeApplicationIpcError } from "@/features/application/errorReference";
import type { ViewSession } from "@/shared/types/plugins/generated";

export type PluginViewFailurePhase = "attach" | "detach" | "navigation" | "bridge";
export interface PluginViewFailure {
  readonly phase: PluginViewFailurePhase;
  readonly code: string;
  readonly incidentId: string | null;
}

class SessionFailure extends Error {
  constructor(readonly failure: PluginViewFailure) {
    super(failure.code);
  }
}

export function pluginViewFailure(
  error: unknown,
  phase: PluginViewFailurePhase,
): PluginViewFailure {
  if (error instanceof SessionFailure) return error.failure;
  const reference = normalizeApplicationIpcError("plugin_view", error);
  const pluginCode = (reference.details as { pluginCode?: unknown } | null)?.pluginCode;
  const code =
    typeof pluginCode === "string" && /^[a-z][a-z0-9_]{0,95}$/.test(pluginCode)
      ? pluginCode
      : reference.code;
  return { phase, code, incidentId: reference.incidentId };
}

export function pluginViewFailureMessage(failure: PluginViewFailure): string {
  if (failure.phase === "detach") return "plugins.viewReleaseFailed";
  if (failure.phase === "navigation") return "plugins.viewNavigationBlocked";
  switch (failure.code) {
    case "plugin_busy":
      return "plugins.viewBusy";
    case "plugin_view_limit":
    case "plugin_resource_exhausted":
      return "plugins.viewCapacityReached";
    case "plugin_start_timeout":
    case "plugin_start_failed":
    case "plugin_process_exited":
    case "plugin_request_timeout":
      return "plugins.viewProcessUnavailable";
    case "plugin_stale_context":
      return "plugins.viewSessionExpired";
    default:
      return "plugins.viewConnectionFailed";
  }
}

/** One page owns one backend lease; obsolete attempts drain before their successor starts. */
export class PluginViewSession {
  private epoch = 0;
  private session: ViewSession | null = null;
  private tail: Promise<void> = Promise.resolve();

  open(pluginId: string, viewId: string): Promise<ViewSession | null> {
    const epoch = ++this.epoch;
    return this.enqueue(async () => {
      await this.release();
      if (epoch !== this.epoch) return null;
      try {
        this.session = await pluginService.attach(pluginId, viewId);
      } catch (error) {
        throw new SessionFailure(pluginViewFailure(error, "attach"));
      }
      if (epoch !== this.epoch) {
        await this.release();
        return null;
      }
      return this.session;
    });
  }

  close(): Promise<void> {
    this.epoch += 1;
    return this.enqueue(() => this.release());
  }

  private async release(): Promise<void> {
    if (!this.session) return;
    // Keep the lease identity if release fails. Retrying must release it, not allocate another slot.
    try {
      await pluginService.detach(this.session.sessionId);
    } catch (error) {
      throw new SessionFailure(pluginViewFailure(error, "detach"));
    }
    this.session = null;
  }

  private enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const result = this.tail.then(operation);
    this.tail = result.then(
      () => undefined,
      () => undefined,
    );
    return result;
  }
}
