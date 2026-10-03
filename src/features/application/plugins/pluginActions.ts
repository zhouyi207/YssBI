import type { TFunction } from "i18next";
import { createOperationId, pluginService } from "@/services/plugins/pluginService";
import type { PluginViewReply } from "@/services/plugins/pluginWireParser";
import { openPathDialog, savePathDialog } from "@/services/platform/pathDialog";
import { revealPath } from "@/services/platform/opener";
import { ui } from "@/features/core/ui/ui";

export async function installLocalPlugin(t: TFunction, isCurrent: () => boolean) {
  if (!isCurrent()) return;
  const selection = await openPathDialog({
    title: t("plugins.installPackage"),
    filters: [{ name: "YssBI Plugin", extensions: ["yssplugin"] }],
  });
  if (!isCurrent()) return;
  if (!selection.ok) throw new Error("plugin_file_dialog_failed");
  if (typeof selection.value !== "string") return;
  const inspection = await pluginService.inspect(selection.value);
  if (!isCurrent()) return;
  let approvedPreviousSigner: string | null = null;
  if (inspection.previousSignerKey && inspection.previousSignerKey !== inspection.signerKey) {
    const changed = await ui.confirm({
      title: t("plugins.signerChangeTitle"),
      message: t("plugins.signerChangeMessage", {
        previous: inspection.previousSignerKey,
        next: inspection.signerKey,
      }),
      confirmText: t("plugins.approveSignerChange"),
      type: "danger",
    });
    if (!changed || !isCurrent()) return;
    approvedPreviousSigner = inspection.previousSignerKey;
  }
  const accepted = await ui.confirm({
    title: t("plugins.trustTitle", { name: inspection.manifest.name }),
    message: t("plugins.trustMessage", {
      publisher: inspection.manifest.publisher,
      permissions: inspection.manifest.permissions.join(", "),
      fingerprint: inspection.signerKey,
    }),
    confirmText: t("plugins.install"),
    type: "danger",
  });
  if (accepted && isCurrent()) {
    const operationId = createOperationId();
    await pluginService.install(
      selection.value,
      inspection.packageDigest,
      operationId,
      approvedPreviousSigner,
    );
  }
}
export async function uninstallPlugin(
  id: string,
  name: string,
  t: TFunction,
  isCurrent: () => boolean,
): Promise<boolean> {
  if (!isCurrent()) return false;
  const accepted = await ui.confirm({
    title: t("plugins.uninstallTitle", { name }),
    message: t("plugins.uninstallMessage"),
    confirmText: t("plugins.uninstall"),
    type: "danger",
  });
  if (!accepted || !isCurrent()) return false;
  await pluginService.uninstall(id);
  return true;
}

export async function fulfillPluginUiRequest(
  sessionId: string,
  reply: PluginViewReply,
  isCurrent: () => boolean,
) {
  if (!isCurrent()) return;
  if (reply.kind === "reveal") return revealPath(reply.value.hostUi.path);
  if (reply.kind === "saveFile") {
    const name = reply.value.hostUi.options?.defaultPath?.split(/[\\/]/).pop();
    const selection = await savePathDialog({
      defaultPath: name,
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });
    if (!isCurrent()) return;
    if (!selection.ok || !selection.value) return selection;
    return { ok: true, value: await pluginService.grantExport(sessionId, selection.value) };
  }
  return reply.value;
}
