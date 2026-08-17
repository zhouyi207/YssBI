import { editorDockviewPort } from '@/features/core/dockview';
import { isGraphOpenInAnyTab } from '@/features/core/layout/graphTabQueries';
import { clearResourceDocumentState } from '@/features/core/resource';
import { logger } from '@/utils/appLogger';
import { unloadGraphDocument } from './graphDocumentUnload';

type GraphDocumentKind = 'event' | 'function';

interface CloseGraphDocumentPanelRequest {
  graphPath: string;
  graphKind: GraphDocumentKind;
  panelInstanceId: string;
  afterPanelRemoved(): void | Promise<void>;
}

/** Close one panel instance before deciding whether its shared graph document can be torn down. */
export async function closeGraphDocumentPanel({
  graphPath,
  graphKind,
  panelInstanceId,
  afterPanelRemoved,
}: CloseGraphDocumentPanelRequest): Promise<void> {
  await editorDockviewPort.remove(panelInstanceId);
  await afterPanelRemoved();

  if (isGraphOpenInAnyTab(graphPath)) return;

  clearResourceDocumentState({ id: graphPath, kind: graphKind });
  void unloadGraphDocument(graphPath).catch((error) => {
    logger.graph.warn(
      `Failed to release graph cache '${graphPath}': ${error instanceof Error ? error.message : String(error)}`,
      'closeGraphDocumentPanel',
    );
  });
}
