import type { TFunction } from "i18next";
import { createOperationId, pluginService } from "@/services/plugins/pluginService";
import { openPathDialog, savePathDialog } from "@/services/platform/pathDialog";
import { revealPath } from "@/services/platform/opener";
import { ui } from "@/features/core/ui/ui";

export async function installLocalPlugin(t: TFunction) {
  const selection = await openPathDialog({
    title: t("plugins.installPackage"),
    filters: [{ name: "YssBI Plugin", extensions: ["yssplugin"] }],
  });
  if (!selection.ok) throw new Error("plugin_file_dialog_failed");
  if (typeof selection.value !== "string") return;
  const inspection = await pluginService.inspect(selection.value);
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
    if (!changed) return;
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
  if (accepted) {
    const operationId = createOperationId();
    await pluginService.install(
      selection.value,
      inspection.packageDigest,
      operationId,
      approvedPreviousSigner,
    );
  }
}
export async function uninstallPlugin(id: string, name: string, t: TFunction): Promise<boolean> {
  const accepted = await ui.confirm({
    title: t("plugins.uninstallTitle", { name }),
    message: t("plugins.uninstallMessage"),
    confirmText: t("plugins.uninstall"),
    type: "danger",
  });
  if (!accepted) return false;
  await pluginService.uninstall(id);
  return true;
}

export async function fulfillPluginUiRequest(sessionId: string, result: unknown) {
  const request = (
    result as { hostUi?: { kind?: string; path?: string; options?: { defaultPath?: string } } }
  )?.hostUi;
  if (request?.kind === "reveal" && request.path) return revealPath(request.path);
  if (request?.kind === "saveFile") {
    const name = request.options?.defaultPath?.split(/[\\/]/).pop();
    const selection = await savePathDialog({
      defaultPath: name,
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });
    if (!selection.ok || !selection.value) return selection;
    return { ok: true, value: await pluginService.grantExport(sessionId, selection.value) };
  }
  return result;
}
