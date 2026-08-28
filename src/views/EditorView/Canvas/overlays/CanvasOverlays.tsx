import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';
import type { NodeCreationDescriptor } from '@/features/domain/nodeCatalog/creationDescriptor';
import type { PortAddressDto } from '@/shared/types/dto/editorProjection';
import { getOverlayPortalRoot } from '@/shared/ui/overlayPortalRoot';
import { ActionMenu } from '@/shared/ui/actionMenu';
import { NodePalette } from '../../Layout/NodePalette';
import { PinResultSearch } from './PinResultSearchPalette';
import { CanvasExecutionToolbar } from './CanvasExecutionToolbar';

export type CanvasOverlayGraphModel =
  | { kind: 'event'; graphPath: string }
  | { kind: 'function'; graphPath: string }
  | { kind: 'unavailable' };

export type CanvasPaletteOverlayModel =
  | { kind: 'hidden' }
  | {
      kind: 'visible';
      x: number;
      y: number;
      graphPath: string | null;
      graphRevision: number | null;
      sourcePort: PortAddressDto | null;
      onSelect: (descriptor: NodeCreationDescriptor, locale: string) => void;
      onClose: () => void;
    };

export type CanvasVariableOverlayModel =
  | { kind: 'hidden' }
  | {
      kind: 'visible';
      x: number;
      y: number;
      variableName: string;
      onGet: () => void;
      onSet: () => void;
      onClose: () => void;
  };


export type CanvasExecutionOverlayModel =
  | { kind: 'hidden' }
  | {
      kind: 'event';
      graphPath: string;
      onExecute: () => void;
      onCancelExecution: () => void;
      onClearArtifacts: () => void;
    };

export interface CanvasOverlaysModel {
  graph: CanvasOverlayGraphModel;
  palette: CanvasPaletteOverlayModel;
  variable: CanvasVariableOverlayModel;
  execution: CanvasExecutionOverlayModel;
}

export default function CanvasOverlays({ model }: { model: CanvasOverlaysModel }) {
  const { t } = useTranslation();
  const { graph, palette, variable, execution } = model;

  return (
    <>
      {graph.kind === 'event' ? (
        <div className="absolute left-3 top-3 z-40">
          <PinResultSearch graphPath={graph.graphPath} />
        </div>
      ) : null}

      {execution.kind === 'event' ? (
        <CanvasExecutionToolbar
          graphPath={execution.graphPath}
          onExecute={execution.onExecute}
          onCancelExecution={execution.onCancelExecution}
          onClearArtifacts={execution.onClearArtifacts}
        />
      ) : null}

      {palette.kind === 'visible'
        ? createPortal(
            <NodePalette
              x={palette.x}
              y={palette.y}
              graphPath={palette.graphPath}
              graphRevision={palette.graphRevision}
              sourcePort={palette.sourcePort}
              onSelect={palette.onSelect}
              onClose={palette.onClose}
            />,
            getOverlayPortalRoot(),
          )
        : null}


      {variable.kind === 'visible' ? (
        <ActionMenu
          position={{ x: variable.x, y: variable.y }}
          sections={[
            {
              items: [
                {
                  id: 'get-variable',
                  label: t('canvas.getVariable', { name: variable.variableName }),
                  onClick: variable.onGet,
                },
              ],
            },
            {
              items: [
                {
                  id: 'set-variable',
                  label: t('canvas.setVariable', { name: variable.variableName }),
                  onClick: variable.onSet,
                },
              ],
            },
          ]}
          onClose={variable.onClose}
        />
      ) : null}
    </>
  );
}
