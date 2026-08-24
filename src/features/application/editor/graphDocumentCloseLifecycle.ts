import { requestCloseWorkbenchPanel } from './workbenchPanelClose';

interface CloseGraphDocumentPanelRequest {
  panelInstanceId: string;
  afterPanelRemoved(): void | Promise<void>;
}

/** Close one graph panel through the shared batch coordinator before applying UI fallback. */
export async function closeGraphDocumentPanel({
  panelInstanceId,
  afterPanelRemoved,
}: CloseGraphDocumentPanelRequest): Promise<boolean> {
  if (!await requestCloseWorkbenchPanel(panelInstanceId)) return false;
  await afterPanelRemoved();
  return true;
}
