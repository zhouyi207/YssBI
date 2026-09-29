import { workbenchLayoutControl, workbenchLayoutRead } from "@/modules/workbench/public";
import { parseReferenceUrl } from "@/shared/utils/referenceUrl";
import { openExternalUrl } from "@/shared/utils/openExternalUrl";

export async function openReferenceLink(href: string, title: string): Promise<void> {
  const url = parseReferenceUrl(href);
  if (!url) return;
  if (!workbenchLayoutRead.isReady) {
    await openExternalUrl(url.href);
    return;
  }
  await workbenchLayoutControl.openReference({
    url: url.href,
    title: title.trim() || url.hostname,
  });
}
