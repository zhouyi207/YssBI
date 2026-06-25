import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { Table, TableBody, TableCell, TableRow } from '@/components/ui/table';
import type { PinData } from '@/shared/types/store/graph';
import { getNodeDefinitionMeta } from '@/shared/types/domain/node';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { useNodeRegistryStore } from '@/features/core/nodeRegister';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { NodeDocumentationPanel } from '../node/NodeDocumentationPanel';
import { NodePinInterfacePanel } from '../node/NodePinInterfacePanel';
import { resolveNodeDocumentationContent } from '../nodeDocumentation';
import { resolveNodePinSpecs } from '../resolveNodePinSpecs';

const EMPTY_PINS: PinData[] = [];

interface NodeDetailPanelProps {
  nodeId: string;
  graphId: string;
}

export function NodeDetailPanel({ nodeId, graphId }: NodeDetailPanelProps) {
  const { t, i18n } = useTranslation();
  const node = useGraphDataStore((s) => s.nodes[nodeId]);
  const pins = useGraphDataStore(
    useShallow((s) => {
      const pinIds = s.nodePins[nodeId];
      if (!pinIds?.length) return EMPTY_PINS;
      return pinIds.map((pid) => s.pins[pid]).filter(Boolean);
    }),
  );
  const nodeType = node?.nodeType;
  const definition = useNodeRegistryStore((s) =>
    nodeType ? s.definitions.get(nodeType) : undefined,
  );

  const pinSpecs = useMemo(
    () => resolveNodePinSpecs(nodeId, pins, definition),
    [nodeId, pins, definition],
  );

  const documentation = useMemo(() => {
    const meta = getNodeDefinitionMeta(definition);
    return resolveNodeDocumentationContent(meta, i18n.language, node?.description);
  }, [definition, node?.description, i18n.language]);

  if (!node) {
    return (
      <DetailPanelShell title={t('detail.titleNode')}>
        <div className="p-4 text-[11px] text-gray-400">{t('detail.nodeNotFound')}</div>
      </DetailPanelShell>
    );
  }

  return (
    <DetailPanelShell title={t('detail.titleWithName', { name: node.title || node.nodeType })}>
      <Table className="text-[11px] text-[#cccccc]">
        <TableBody>
          <TableRow>
            <TableCell className="w-20 bg-white/5 font-bold text-gray-400">
              {t('detail.fields.name')}
            </TableCell>
            <TableCell className="font-medium text-gray-200">{node.title}</TableCell>
          </TableRow>
          <TableRow>
            <TableCell className="bg-white/5 font-bold text-gray-400">
              {t('detail.fields.type')}
            </TableCell>
            <TableCell className="font-mono text-[10px] text-gray-400">{node.nodeType}</TableCell>
          </TableRow>
          {node.category?.length > 0 && (
            <TableRow>
              <TableCell className="bg-white/5 font-bold text-gray-400">
                {t('detail.fields.category')}
              </TableCell>
              <TableCell className="text-gray-400">{node.category.join(' / ')}</TableCell>
            </TableRow>
          )}
          <TableRow>
            <TableCell className="bg-white/5 font-bold text-gray-400">
              {t('detail.fields.graph')}
            </TableCell>
            <TableCell className="font-mono text-[10px] text-gray-500">{graphId}</TableCell>
          </TableRow>
        </TableBody>
      </Table>
      <NodePinInterfacePanel inputs={pinSpecs.inputs} outputs={pinSpecs.outputs} />
      {documentation && <NodeDocumentationPanel markdown={documentation} />}
    </DetailPanelShell>
  );
}
